// SPDX-License-Identifier: MIT OR Apache-2.0
//! Supervision for explicitly trusted native providers. This is not a sandbox.
use crate::ExternalError;
pub use crate::invocation::Invocation;
use permesh_core::Snapshot;
use permesh_provider_protocol::{
    DiscoveryDecoder, HealthDecoder, MAX_FRAME_BYTES, MAX_TRANSCRIPT_BYTES, Progress,
    handshake_request, handshake_request_versioned,
};
use permesh_provider_sdk::Capability;
use permesh_provider_sdk::Health;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use std::{future::Future, io, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{ChildStdin, Command},
    time::{Instant, sleep_until},
};
use zeroize::{Zeroize, Zeroizing};

const STDERR_LIMIT: usize = 64 * 1024;
#[derive(Clone, Copy)]
struct Deadlines {
    handshake: Duration,
    operation: Duration,
    cancel: Duration,
    cleanup: Duration,
}
impl Default for Deadlines {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(5),
            operation: Duration::from_secs(60),
            cancel: Duration::from_secs(1),
            cleanup: Duration::from_secs(5),
        }
    }
}

/// Execute one discovery in a private temporary working directory with no inherited
/// credentials or arguments. Cancellation is cooperative briefly, then forcible and
/// awaited. Callers should signal cancellation rather than dropping this future.
pub async fn discover(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    cancellation: impl Future<Output = ()>,
) -> Result<Snapshot, ExternalError> {
    discover_with_deadlines(
        executable,
        provider,
        instance,
        capabilities,
        cancellation,
        Deadlines::default(),
    )
    .await
}

struct Supervised(Box<dyn ChildWrapper>, bool);
impl Supervised {
    fn terminate(&mut self) -> Result<(), ExternalError> {
        if !self.1 {
            return Ok(());
        }
        terminate(&mut *self.0)?;
        self.1 = false;
        Ok(())
    }
}
impl Drop for Supervised {
    fn drop(&mut self) {
        // A fallback for callers that abandon the future. Explicit paths below also
        // await reaping. On Unix descendants can deliberately leave this group.
        if self.1 {
            match self.0.start_kill() {
                Ok(()) => {}
                Err(_) => {
                    // Drop cannot return an error or await reaping. The native
                    // kill-on-drop fallback remains armed; explicit cleanup
                    // paths above the guard report failures to the caller.
                }
            }
        }
    }
}

async fn discover_with_deadlines(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    cancellation: impl Future<Output = ()>,
    deadlines: Deadlines,
) -> Result<Snapshot, ExternalError> {
    let decoder = DiscoveryDecoder::new(provider, instance, Some(capabilities))
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request(instance).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: None,
            method: "discover",
            version: 1,
        },
        cancellation,
        deadlines,
    )
    .await
}

trait Decoder {
    type Output;
    fn push(&mut self, frame: &[u8]) -> Result<Progress, permesh_provider_protocol::ProtocolError>;
    fn finish(self) -> Result<Self::Output, permesh_provider_protocol::ProtocolError>;
}
impl Decoder for DiscoveryDecoder {
    type Output = Snapshot;
    fn push(&mut self, frame: &[u8]) -> Result<Progress, permesh_provider_protocol::ProtocolError> {
        self.push_frame(frame)
    }
    fn finish(self) -> Result<Snapshot, permesh_provider_protocol::ProtocolError> {
        self.finish()
    }
}
impl Decoder for HealthDecoder {
    type Output = Health;
    fn push(&mut self, frame: &[u8]) -> Result<Progress, permesh_provider_protocol::ProtocolError> {
        self.push_frame(frame)
    }
    fn finish(self) -> Result<Health, permesh_provider_protocol::ProtocolError> {
        self.finish()
    }
}
struct Exchange<'a, D> {
    decoder: D,
    handshake: Vec<u8>,
    invocation: Option<&'a Invocation>,
    method: &'static str,
    version: u32,
}

/// Discover using draft2; credentials are delivered only after identity/capability validation.
pub async fn discover_configured(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    invocation: &Invocation,
    cancellation: impl Future<Output = ()>,
) -> Result<Snapshot, ExternalError> {
    configured_discovery(
        executable,
        provider,
        instance,
        capabilities,
        invocation,
        cancellation,
        Deadlines::default(),
    )
    .await
}
async fn configured_discovery(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    invocation: &Invocation,
    cancellation: impl Future<Output = ()>,
    deadlines: Deadlines,
) -> Result<Snapshot, ExternalError> {
    let decoder = DiscoveryDecoder::new_versioned(provider, instance, Some(capabilities), 2)
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request_versioned(instance, 2).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: Some(invocation),
            method: "discover",
            version: 2,
        },
        cancellation,
        deadlines,
    )
    .await
}
/// Perform only the draft2 health operation, without requesting discovery records.
pub async fn check_configured(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    invocation: &Invocation,
    cancellation: impl Future<Output = ()>,
) -> Result<Health, ExternalError> {
    let decoder = HealthDecoder::new(provider, instance, Some(capabilities))
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request_versioned(instance, 2).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: Some(invocation),
            method: "check",
            version: 2,
        },
        cancellation,
        Deadlines::default(),
    )
    .await
}

async fn supervise<D: Decoder>(
    executable: &Path,
    exchange_spec: Exchange<'_, D>,
    cancellation: impl Future<Output = ()>,
    deadlines: Deadlines,
) -> Result<D::Output, ExternalError> {
    let version = exchange_spec.version;
    if !executable.is_absolute() {
        return Err(ExternalError::Input);
    }
    #[cfg(not(windows))]
    let directory = tempfile::tempdir().map_err(|_| ExternalError::Storage)?;
    #[cfg(windows)]
    let directory = crate::trust::private_working_directory()?;
    let mut command = Command::new(executable);
    command
        .env_clear()
        .current_dir(directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    let mut command = CommandWrap::from(command);
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::tokio::JobObject);
    let mut child = Supervised(
        command.spawn().map_err(|error| {
            #[cfg(test)]
            eprintln!(
                "native spawn failed: kind={:?}, os_code={:?}",
                error.kind(),
                error.raw_os_error()
            );
            #[cfg(not(test))]
            drop(error);
            ExternalError::Spawn
        })?,
        true,
    );
    let pipes = (
        child.0.stdin().take(),
        child.0.stdout().take(),
        child.0.stderr().take(),
    );
    let (Some(input), Some(mut stdout), Some(mut stderr)) = pipes else {
        let process_cleanup = cleanup(&mut child, deadlines.cleanup).await;
        let directory_cleanup = directory.close().map_err(|_| ExternalError::Cleanup);
        process_cleanup?;
        directory_cleanup?;
        return Err(ExternalError::Spawn);
    };
    let mut stdin = Some(input);
    let mut stdout_total = 0usize;
    let mut stderr_total = 0usize;
    let started = Instant::now();
    let result = {
        let operation = async {
            let (snapshot, (), status) = tokio::try_join!(
                exchange(
                    &mut stdin,
                    &mut stdout,
                    &mut stdout_total,
                    exchange_spec,
                    started + deadlines.handshake
                ),
                discard_stream(
                    &mut stderr,
                    &mut stderr_total,
                    STDERR_LIMIT,
                    ExternalError::StderrLimit
                ),
                async {
                    // Exactly one child wrapper (group/job) sits above the native
                    // child. Observe its leader without waiting on descendants.
                    let status = child
                        .0
                        .inner_mut()
                        .wait()
                        .await
                        .map_err(|_| ExternalError::Exit)?;
                    child.terminate()?;
                    Ok(status)
                }
            )?;
            if !status.success() {
                return Err(ExternalError::Exit);
            }
            Ok(snapshot)
        };
        tokio::pin!(operation);
        tokio::pin!(cancellation);
        tokio::select! {
            biased;
            () = &mut cancellation => Err(ExternalError::Cancelled),
            () = sleep_until(started + deadlines.operation) => Err(ExternalError::Timeout),
            result = &mut operation => result,
        }
    };
    if matches!(result, Err(ExternalError::Cancelled)) {
        let until = Instant::now() + deadlines.cancel;
        let cooperative = async {
            tokio::try_join!(
                async {
                    if let Some(input) = &mut stdin {
                        input
                            .write_all(if version == 2 {
                                b"{\"protocol\":2,\"id\":\"cancel\",\"method\":\"cancel\"}\n"
                            } else {
                                b"{\"protocol\":1,\"id\":\"cancel\",\"method\":\"cancel\"}\n"
                            })
                            .await
                            .map_err(|_| ExternalError::Protocol)?;
                        input.flush().await.map_err(|_| ExternalError::Protocol)?;
                    }
                    stdin.take();
                    child
                        .0
                        .inner_mut()
                        .wait()
                        .await
                        .map_err(|_| ExternalError::Exit)?;
                    Ok(())
                },
                discard_stream(
                    &mut stdout,
                    &mut stdout_total,
                    MAX_TRANSCRIPT_BYTES,
                    ExternalError::Protocol
                ),
                discard_stream(
                    &mut stderr,
                    &mut stderr_total,
                    STDERR_LIMIT,
                    ExternalError::StderrLimit
                )
            )
        };
        match tokio::time::timeout_at(until, cooperative).await {
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => {
                // A rejected cancel, failed pipe, or expired grace never skips
                // forcible termination and awaited cleanup below. Cancellation
                // remains the result unless that cleanup itself fails.
            }
        }
    }
    stdin.take();
    let process_cleanup = cleanup(&mut child, deadlines.cleanup).await;
    let directory_cleanup = directory.close().map_err(|_| ExternalError::Cleanup);
    process_cleanup?;
    directory_cleanup?;
    result
}

async fn cleanup(child: &mut Supervised, duration: Duration) -> Result<(), ExternalError> {
    let cleanup_deadline = Instant::now() + duration;
    let killed = child.terminate();
    tokio::time::timeout_at(cleanup_deadline, child.0.inner_mut().wait())
        .await
        .map_err(|_| ExternalError::Cleanup)?
        .map_err(|_| ExternalError::Cleanup)?;
    if killed.is_err() {
        // macOS can reject killpg while the last member is a zombie. Reap
        // our leader first, then require a successful termination/absent group.
        child.terminate()?;
    }
    // process-wrap 10's Windows completion-port wait may consume a nonterminal
    // job notification. We guarantee job termination was requested and the
    // direct leader was reaped above; we do not claim to reap all descendants.
    tokio::time::timeout_at(cleanup_deadline, child.0.wait())
        .await
        .map_err(|_| ExternalError::Cleanup)?
        .map_err(|_| ExternalError::Cleanup)?;
    Ok(())
}

fn terminate(child: &mut dyn ChildWrapper) -> Result<(), ExternalError> {
    match child.start_kill() {
        Ok(()) => Ok(()),
        // ESRCH means the Unix group no longer has any processes.
        #[cfg(unix)]
        Err(error) if error.raw_os_error() == Some(3) => Ok(()),
        Err(_) => Err(ExternalError::Cleanup),
    }
}

async fn exchange<D: Decoder>(
    stdin: &mut Option<ChildStdin>,
    mut stdout: impl AsyncRead + Unpin,
    total: &mut usize,
    spec: Exchange<'_, D>,
    handshake_deadline: Instant,
) -> Result<D::Output, ExternalError> {
    let Exchange {
        mut decoder,
        handshake,
        invocation,
        method,
        ..
    } = spec;
    let mut frame = Zeroizing::new(Vec::new());
    let mut chunk = [0u8; 8192];
    let mut negotiated = false;
    tokio::time::timeout_at(
        handshake_deadline,
        stdin
            .as_mut()
            .ok_or(ExternalError::Protocol)?
            .write_all(&handshake),
    )
    .await
    .map_err(|_| ExternalError::Timeout)?
    .map_err(|_| ExternalError::Protocol)?;
    loop {
        let read_limit = chunk
            .len()
            .min(MAX_TRANSCRIPT_BYTES.saturating_sub(*total) + 1);
        let length = if negotiated {
            stdout.read(&mut chunk[..read_limit]).await
        } else {
            tokio::time::timeout_at(handshake_deadline, stdout.read(&mut chunk[..read_limit]))
                .await
                .map_err(|_| ExternalError::Timeout)?
        }
        .map_err(|_| ExternalError::Protocol)?;
        *total += length;
        if *total > MAX_TRANSCRIPT_BYTES {
            return Err(ExternalError::Protocol);
        }
        if length == 0 {
            if !frame.is_empty() {
                return Err(ExternalError::Protocol);
            }
            return decoder.finish().map_err(|_| ExternalError::Protocol);
        }
        for byte in &chunk[..length] {
            if frame.len() == MAX_FRAME_BYTES {
                return Err(ExternalError::Protocol);
            }
            frame.push(*byte);
            if *byte == b'\n' {
                if let Some(invocation) = invocation
                    && invocation.rejects_reflection(&frame)?
                {
                    return Err(ExternalError::Protocol);
                }
                match decoder.push(&frame).map_err(|_| ExternalError::Protocol)? {
                    Progress::Handshake => {
                        negotiated = true;
                        let request = match invocation {
                            Some(invocation) => invocation.request(method)?,
                            None => Zeroizing::new(
                                b"{\"protocol\":1,\"id\":\"discover\",\"method\":\"discover\"}\n"
                                    .to_vec(),
                            ),
                        };
                        stdin
                            .as_mut()
                            .ok_or(ExternalError::Protocol)?
                            .write_all(&request)
                            .await
                            .map_err(|_| ExternalError::Protocol)?;
                    }
                    Progress::Complete => {
                        stdin.take();
                    }
                    Progress::Record => {}
                }
                frame.zeroize();
            }
        }
    }
}

async fn discard_stream(
    mut stream: impl AsyncRead + Unpin,
    total: &mut usize,
    limit: usize,
    exceeded: ExternalError,
) -> Result<(), ExternalError> {
    let mut chunk = [0u8; 8192];
    loop {
        let read_limit = chunk.len().min(limit.saturating_sub(*total) + 1);
        let count = stream
            .read(&mut chunk[..read_limit])
            .await
            .map_err(|_: io::Error| ExternalError::Protocol)?;
        if count == 0 {
            return Ok(());
        }
        *total += count;
        if *total > limit {
            return Err(exceeded);
        }
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
