//! File I/O for molecular dynamics

use crate::types::{Atom, SimulationBox};
use crate::error::{FxnnError, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Write atoms to XYZ format
pub fn write_xyz<P: AsRef<Path>>(path: P, atoms: &[Atom], comment: &str) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", atoms.len())?;
    writeln!(writer, "{}", comment)?;
    for atom in atoms {
        writeln!(writer, "Ar {:12.6} {:12.6} {:12.6}", atom.position[0], atom.position[1], atom.position[2])?;
    }
    Ok(())
}

/// Append XYZ frame
pub fn append_xyz<P: AsRef<Path>>(path: P, atoms: &[Atom], step: usize) -> Result<()> {
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", atoms.len())?;
    writeln!(writer, "Step {}", step)?;
    for atom in atoms {
        writeln!(writer, "Ar {:12.6} {:12.6} {:12.6}", atom.position[0], atom.position[1], atom.position[2])?;
    }
    Ok(())
}
