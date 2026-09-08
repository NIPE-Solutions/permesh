// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::output::{field, safe};
use serde_json::Value;
use std::io::{self, Write};
pub fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    writeln!(
        out,
        "Provider packages · {}",
        safe(field(result, "provider"))
    )?;
    writeln!(out, "\n  Platform  {}", safe(field(result, "target")))?;
    writeln!(
        out,
        "  Available {}",
        safe(field(result, "available_version"))
    )?;
    writeln!(
        out,
        "  Selected  {}",
        safe(field(&result["selected"], "version"))
    )?;
    writeln!(out, "\n{}", safe(field(result, "message")))?;
    if result["package"].is_object() {
        writeln!(
            out,
            "\nExecutable\n  {}",
            safe(field(&result["package"], "executable"))
        )?;
        writeln!(
            out,
            "\nSHA-256\n  {}",
            safe(field(&result["selected"], "executable_sha256"))
        )?;
        writeln!(
            out,
            "\nNext\n  Review this release and its capabilities, then use:\n  permesh provider external trust --help\n\n  Existing workspaces keep their pinned version until you edit and approve them."
        )?;
    }
    Ok(())
}
