// SPDX-License-Identifier: MIT
use crate::args::Cli;
use clap::CommandFactory;
use clap_complete::Shell;
use std::io::{self, Write};

pub fn write(shell: Shell, writer: &mut impl Write) -> io::Result<()> {
    // Upstream generators panic on writer errors. Vec writing is infallible;
    // only our fallible final write reaches stdout (including closed pipes).
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "permesh", &mut script);
    writer.write_all(&script)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter {
        fail_on_flush: bool,
    }
    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_on_flush {
                Ok(bytes.len())
            } else {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::ConnectionReset.into())
        }
    }

    #[test]
    fn write_and_flush_failures_are_returned_without_panicking() {
        for (fail_on_flush, kind) in [
            (false, io::ErrorKind::PermissionDenied),
            (true, io::ErrorKind::ConnectionReset),
        ] {
            let result = write(Shell::Bash, &mut FailingWriter { fail_on_flush });
            assert!(matches!(result, Err(error) if error.kind() == kind));
        }
    }
}
