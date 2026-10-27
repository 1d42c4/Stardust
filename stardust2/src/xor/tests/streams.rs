use super::*;
use crate::xor::test_support::data;
use std::io::Cursor;

fn xor(a: &[u8], key: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    assert_eq!(
        xor_streams(
            &mut Cursor::new(a),
            &mut Cursor::new(key),
            &mut out,
            a.len() as u64
        )?,
        a.len() as u64
    );
    Ok(out)
}

struct Choppy<'a> {
    input: &'a [u8],
    chunk: usize,
    calls: usize,
    interrupt: bool,
}
impl Read for Choppy<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.interrupt && self.calls % 2 == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = buf.len().min(self.chunk);
        self.input.read(&mut buf[..n])
    }
}

struct FailRead {
    input: Cursor<Vec<u8>>,
    after: u64,
}
impl Read for FailRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.after.saturating_sub(self.input.position());
        if left == 0 {
            return Err(io::Error::other("injected read failure"));
        }
        let n = (buf.len() as u64).min(left) as usize;
        self.input.read(&mut buf[..n])
    }
}

struct ShortWrite {
    bytes: Vec<u8>,
    calls: usize,
}
impl Write for ShortWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls % 2 == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = buf.len().min(3);
        self.bytes.extend_from_slice(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FailWrite {
    left: usize,
    zero: bool,
}
impl Write for FailWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.left == 0 {
            return if self.zero {
                Ok(0)
            } else {
                Err(io::Error::other("injected write failure"))
            };
        }
        let n = buf.len().min(self.left);
        self.left -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn known_vector() {
    assert_eq!(xor(&[255, 15, 170], &[15, 15, 85]).unwrap(), [240, 0, 255]);
}
#[test]
fn all_byte_values() {
    assert_eq!(
        xor(
            &(0..=255).collect::<Vec<u8>>(),
            &(0..=255).rev().collect::<Vec<u8>>()
        )
        .unwrap(),
        vec![255; 256]
    );
}
#[test]
fn two_empty_inputs() {
    assert!(xor(&[], &[]).unwrap().is_empty());
}
#[test]
fn empty_a_and_nonempty_key() {
    assert!(xor(&[], &[1, 2]).unwrap().is_empty());
}
#[test]
fn longer_key_uses_only_its_prefix() {
    assert_eq!(xor(&[1, 2], &[4, 8, 99]).unwrap(), [5, 10]);
}
#[test]
fn equal_inputs_produce_zeros() {
    let a = data(BUF_SIZE + 7, 1);
    assert_eq!(xor(&a, &a).unwrap(), vec![0; a.len()]);
}
#[test]
fn zero_key_preserves_all_bytes() {
    let a = data(BUF_SIZE + 1, 2);
    assert_eq!(xor(&a, &vec![0; a.len()]).unwrap(), a);
}
#[test]
fn empty_key_fails_for_nonempty_a() {
    assert_eq!(
        xor(&[1], &[]).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}
#[test]
fn key_short_by_one_fails() {
    assert_eq!(
        xor(&[1, 2, 3], &[4, 5]).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn boundaries_and_multiple_buffers_match_independent_oracle() {
    for length in [
        1,
        2,
        8191,
        8192,
        8193,
        BUF_SIZE - 1,
        BUF_SIZE,
        BUF_SIZE + 1,
        BUF_SIZE * 2,
        BUF_SIZE * 3 + 17,
    ] {
        let a = data(length, 13);
        let key = data(length + 5, 91);
        let expected: Vec<u8> = a.iter().zip(&key).map(|(x, y)| x ^ y).collect();
        assert_eq!(xor(&a, &key).unwrap(), expected, "length {length}");
    }
}

#[test]
fn randomized_oracles_and_round_trips() {
    for seed in 1..=256 {
        let length = (seed * 929 % 140003) as usize;
        let a = data(length, seed);
        let key = data(length + (seed % 131) as usize, seed + 999);
        let expected: Vec<u8> = a.iter().zip(&key).map(|(x, y)| x ^ y).collect();
        let masked = xor(&a, &key).unwrap();
        assert_eq!(masked, expected, "seed {seed}");
        assert_eq!(xor(&masked, &key).unwrap(), a, "seed {seed}");
    }
}

#[test]
fn short_and_interrupted_reads_use_all_bytes() {
    let a = data(BUF_SIZE + 33, 7);
    let key = data(a.len() + 12, 8);
    for interrupt in [false, true] {
        let mut left = Choppy {
            input: &a,
            chunk: 13,
            calls: 0,
            interrupt,
        };
        let mut right = Choppy {
            input: &key,
            chunk: 7,
            calls: 0,
            interrupt,
        };
        let mut out = Vec::new();
        xor_streams(&mut left, &mut right, &mut out, a.len() as u64).unwrap();
        assert_eq!(out, xor(&a, &key).unwrap());
    }
}

#[test]
fn short_and_interrupted_writes_are_completed() {
    let a = data(BUF_SIZE + 7, 5);
    let key = data(a.len(), 8);
    let mut out = ShortWrite {
        bytes: Vec::new(),
        calls: 0,
    };
    xor_streams(
        &mut a.as_slice(),
        &mut key.as_slice(),
        &mut out,
        a.len() as u64,
    )
    .unwrap();
    assert_eq!(out.bytes, xor(&a, &key).unwrap());
}

#[test]
fn zero_writes_fail_instead_of_spinning() {
    let error = xor_streams(
        &mut &[1, 2][..],
        &mut &[3, 4][..],
        &mut FailWrite {
            left: 0,
            zero: true,
        },
        2,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
}

#[test]
fn errors_after_partial_writes_propagate() {
    let a = vec![1; BUF_SIZE * 2];
    let error = xor_streams(
        &mut a.as_slice(),
        &mut a.as_slice(),
        &mut FailWrite {
            left: BUF_SIZE + 10,
            zero: false,
        },
        a.len() as u64,
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected write"));
}

#[test]
fn data_read_errors_propagate_after_a_full_buffer() {
    let a = vec![1; BUF_SIZE * 2];
    let mut failing = FailRead {
        input: Cursor::new(a.clone()),
        after: BUF_SIZE as u64 + 3,
    };
    let mut out = Vec::new();
    let error = xor_streams(&mut failing, &mut a.as_slice(), &mut out, a.len() as u64).unwrap_err();
    assert!(error.to_string().contains("FILE_A"));
    assert_eq!(out.len(), BUF_SIZE);
}

#[test]
fn key_read_errors_propagate_after_a_full_buffer() {
    let a = vec![1; BUF_SIZE * 2];
    let mut failing = FailRead {
        input: Cursor::new(a.clone()),
        after: BUF_SIZE as u64 + 3,
    };
    let mut out = Vec::new();
    let error = xor_streams(&mut a.as_slice(), &mut failing, &mut out, a.len() as u64).unwrap_err();
    assert!(error.to_string().contains("key.key"));
    assert_eq!(out.len(), BUF_SIZE);
}

#[test]
fn shortened_a_fails() {
    assert_eq!(
        xor_streams(&mut &[1][..], &mut &[2, 3][..], &mut Vec::new(), 2)
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn growth_of_a_fails_even_from_empty() {
    for length in [0, 2] {
        assert!(
            xor_streams(
                &mut &[1, 2, 3][..],
                &mut &[4, 5, 6][..],
                &mut Vec::new(),
                length
            )
            .unwrap_err()
            .to_string()
            .contains("grew")
        );
    }
}

#[test]
fn final_eof_check_propagates_read_errors() {
    let mut failing = FailRead {
        input: Cursor::new(vec![1, 2]),
        after: 2,
    };
    assert!(xor_streams(&mut failing, &mut &[3, 4][..], &mut Vec::new(), 2).is_err());
}

#[test]
fn key_tail_is_never_read() {
    let mut key = FailRead {
        input: Cursor::new(vec![1, 2]),
        after: 2,
    };
    let mut out = Vec::new();
    xor_streams(&mut &[4, 8][..], &mut key, &mut out, 2).unwrap();
    assert_eq!(out, [5, 10]);
}

#[test]
fn virtual_key_larger_than_four_gib_is_not_eagerly_read() {
    struct HugeKey {
        remaining: u64,
        consumed: u64,
        largest_read: usize,
    }
    impl Read for HugeKey {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.largest_read = self.largest_read.max(buffer.len());
            let n = self.remaining.min(buffer.len() as u64) as usize;
            buffer[..n].fill(0xAA);
            self.remaining -= n as u64;
            self.consumed += n as u64;
            Ok(n)
        }
    }
    let mut key = HugeKey {
        remaining: (1_u64 << 32) + 99,
        consumed: 0,
        largest_read: 0,
    };
    let a = vec![0x55; BUF_SIZE + 7];
    let mut out = Vec::new();
    xor_streams(&mut a.as_slice(), &mut key, &mut out, a.len() as u64).unwrap();
    assert_eq!(out, vec![0xFF; a.len()]);
    assert_eq!(key.consumed, a.len() as u64);
    assert!(key.largest_read <= BUF_SIZE);
}
