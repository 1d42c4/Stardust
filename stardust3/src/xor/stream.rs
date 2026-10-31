//! Bounded-memory XOR; never cycle the key or read its unused tail.
use std::io::{self, Read, Write};
pub(super) const BUF_SIZE: usize = 64 * 1024;

pub(super) fn xor_streams(
    a: &mut impl Read,
    key: &mut impl Read,
    out: &mut impl Write,
    length: u64,
) -> io::Result<u64> {
    let mut buf_a = vec![0; BUF_SIZE];
    let mut buf_key = vec![0; BUF_SIZE];
    let mut remaining = length;
    while remaining != 0 {
        let n = remaining.min(BUF_SIZE as u64) as usize;
        a.read_exact(&mut buf_a[..n])
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read FILE_A: {e}")))?;
        key.read_exact(&mut buf_key[..n])
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read key.key: {e}")))?;
        for (x, y) in buf_a[..n].iter_mut().zip(&buf_key[..n]) {
            *x ^= *y;
        }
        out.write_all(&buf_a[..n])?;
        remaining -= n as u64;
    }
    // Detect A growing; never read the key's unused tail.
    loop {
        match a.read(&mut [0]) {
            Ok(0) => return Ok(length),
            Ok(_) => return Err(io::Error::other("FILE_A grew while processing")),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}
