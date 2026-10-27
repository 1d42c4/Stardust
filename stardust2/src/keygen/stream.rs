//! Persistent stream expansion with bounded buffers and checked counters.
use super::model::*;
use chacha20::ChaCha20;
use ctr::cipher::{KeyIvInit, StreamCipher};
use std::io::Write;
use zeroize::Zeroizing;

pub(super) fn write_stream_with_buffer<W: Write>(
    writer: &mut W,
    key: &[u8; DERIVED_KEY_BYTES],
    size: u64,
    buffer_size: usize,
) -> Result<(), GenerateError> {
    let iv = [0_u8; AES_BLOCK_BYTES as usize];
    let cipher = Aes256Ctr::new(key.into(), &iv.into());
    write_cipher_stream_with_buffer(writer, cipher, size, buffer_size)
}

pub(super) fn write_stream_with_generator<W: Write>(
    writer: &mut W,
    key: &[u8; DERIVED_KEY_BYTES],
    size: u64,
    buffer_size: usize,
    stream_generator: StreamGenerator,
) -> Result<(), GenerateError> {
    match stream_generator {
        StreamGenerator::Aes256Ctr => write_stream_with_buffer(writer, key, size, buffer_size),
        StreamGenerator::ChaCha20 => {
            write_chacha20_stream_with_buffer(writer, key, size, buffer_size)
        }
        StreamGenerator::Blake3Xof => write_blake3_xof_with_buffer(writer, key, size, buffer_size),
    }
}

pub(super) fn write_chacha20_stream_with_buffer<W: Write>(
    writer: &mut W,
    key: &[u8; DERIVED_KEY_BYTES],
    size: u64,
    buffer_size: usize,
) -> Result<(), GenerateError> {
    let nonce = [0_u8; CHACHA_NONCE_BYTES];
    let cipher = ChaCha20::new(key.into(), &nonce.into());
    write_cipher_stream_with_buffer(writer, cipher, size, buffer_size)
}

pub(super) fn write_cipher_stream_with_buffer<W, C>(
    writer: &mut W,
    mut cipher: C,
    size: u64,
    buffer_size: usize,
) -> Result<(), GenerateError>
where
    W: Write,
    C: StreamCipher,
{
    validate_size(size)?;
    if buffer_size == 0 {
        return Err(GenerateError::InvalidConfiguration(
            "stream buffer size cannot be zero",
        ));
    }

    let allocation = usize::try_from(size.min(buffer_size as u64))
        .expect("the selected buffer length always fits usize");
    let mut buffer = Zeroizing::new(vec![0_u8; allocation]);
    let mut remaining = size;

    while remaining != 0 {
        let count = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("the selected chunk length always fits usize");
        buffer[..count].fill(0);
        cipher
            .try_apply_keystream(&mut buffer[..count])
            .map_err(|_| GenerateError::CounterExhausted)?;
        writer.write_all(&buffer[..count])?;
        remaining -= count as u64;
    }
    Ok(())
}

pub(super) fn write_blake3_xof_with_buffer<W: Write>(
    writer: &mut W,
    key: &[u8; DERIVED_KEY_BYTES],
    size: u64,
    buffer_size: usize,
) -> Result<(), GenerateError> {
    validate_size(size)?;
    if buffer_size == 0 {
        return Err(GenerateError::InvalidConfiguration(
            "stream buffer size cannot be zero",
        ));
    }

    let allocation = usize::try_from(size.min(buffer_size as u64))
        .expect("the selected buffer length always fits usize");
    let mut buffer = Zeroizing::new(vec![0_u8; allocation]);
    let mut hasher = Zeroizing::new(blake3::Hasher::new_keyed(key));
    hasher.update(BLAKE3_XOF_FORMAT_TAG);
    let mut output_reader = Zeroizing::new(hasher.finalize_xof());
    drop(hasher);
    let mut remaining = size;

    while remaining != 0 {
        let count = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("the selected chunk length always fits usize");
        output_reader.fill(&mut buffer[..count]);
        writer.write_all(&buffer[..count])?;
        remaining -= count as u64;
    }
    Ok(())
}
