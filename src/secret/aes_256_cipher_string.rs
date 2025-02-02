use std::fmt;

use super::{aes_256_cipher::Aes256Cipher, cryp_dec::CrypDec};

struct Aes256CipherString {
    byte_cipher: Aes256Cipher,
}

impl Aes256CipherString {
    pub fn new(key: [u8; 32]) -> Self {
        let byte_cipher = Aes256Cipher::new(key);
        Aes256CipherString { byte_cipher }
    }

    // Private method to pad bytes to a multiple of 16
    fn pad_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        let block_size = 16;
        let padding_length = block_size - (bytes.len() % block_size);
        let mut padded_bytes = bytes.to_vec();
        padded_bytes.extend(vec![padding_length as u8; padding_length]);
        padded_bytes
    }

    // Private method to remove padding from bytes
    fn unpad_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        if bytes.is_empty() {
            return Vec::new(); // Return empty vector if input is empty
        }

        let padding_length = bytes[bytes.len() - 1] as usize;

        // Ensure the padding length is valid
        if padding_length == 0 || padding_length > bytes.len() {
            return bytes.to_vec(); // Return the original bytes if padding is invalid
        }

        bytes[..bytes.len() - padding_length].to_vec()
    }
}

// Define error type for encryption/decryption
#[derive(Debug)]
pub enum CrypDecStringError {
    InvalidLength,
    Utf8Error(std::string::FromUtf8Error),
}

// Implement `std::fmt::Display` for `CrypDecStringError`
impl fmt::Display for CrypDecStringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CrypDecStringError::InvalidLength => write!(f, "Invalid Length"),
            CrypDecStringError::Utf8Error(e) => write!(f, "UTF-8 Error: {}", e),
        }
    }
}

impl std::error::Error for CrypDecStringError {}

// Implement the CrypDec trait for Aes256CipherString
impl CrypDec for Aes256CipherString {
    type Input = String;
    type Output = String;
    type Error = CrypDecStringError;

    fn encrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error> {
        // Convert the string to bytes
        let bytes = data.as_bytes();

        // Pad the bytes to a multiple of 16
        let padded_bytes = self.pad_bytes(bytes);

        // Encrypt each 16-byte block
        let mut encrypted_bytes = Vec::new();
        for chunk in padded_bytes.chunks(16) {
            let block: [u8; 16] = chunk.try_into().unwrap();
            let encrypted_block = self
                .byte_cipher
                .encrypt(&block)
                .map_err(|_| CrypDecStringError::InvalidLength)?;
            encrypted_bytes.extend_from_slice(&encrypted_block);
        }

        // Convert the encrypted bytes to a base64-encoded string
        Ok(base64::encode(encrypted_bytes))
    }

    fn decrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error> {
        // Decode the base64-encoded string to bytes
        let encrypted_bytes =
            base64::decode(data).map_err(|_| CrypDecStringError::InvalidLength)?;

        // Decrypt each 16-byte block
        let mut decrypted_bytes = Vec::new();
        for chunk in encrypted_bytes.chunks(16) {
            let block: [u8; 16] = chunk.try_into().unwrap();
            let decrypted_block = self
                .byte_cipher
                .decrypt(&block)
                .map_err(|_| CrypDecStringError::InvalidLength)?;
            decrypted_bytes.extend_from_slice(&decrypted_block);
        }

        // Remove padding and convert bytes to a string
        let unpadded_bytes = self.unpad_bytes(&decrypted_bytes);
        String::from_utf8(unpadded_bytes).map_err(CrypDecStringError::Utf8Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32]; // Using a zeroed key for simplicity
        let aes_cipher_string = Aes256CipherString::new(key);

        let plaintext = String::from("Hello, world!");

        // Encrypt the plaintext
        let ciphertext = aes_cipher_string.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher_string.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);
    }

    #[test]
    fn test_encrypt_decrypt_empty_string() {
        let key = [0u8; 32];
        let aes_cipher_string = Aes256CipherString::new(key);

        let plaintext = String::from("");

        // Encrypt the plaintext
        let ciphertext = aes_cipher_string.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher_string.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);
    }

    #[test]
    fn test_encrypt_decrypt_string_not_multiple_of_16() {
        let key = [0u8; 32];
        let aes_cipher_string = Aes256CipherString::new(key);

        let plaintext = String::from("This is a test string.");

        // Encrypt the plaintext
        let ciphertext = aes_cipher_string.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher_string.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);
    }

    #[test]
    fn test_encrypt_decrypt_string_multiple_of_16() {
        let key = [0u8; 32];
        let aes_cipher_string = Aes256CipherString::new(key);

        let plaintext = String::from("1234567890123456"); // Exactly 16 bytes

        // Encrypt the plaintext
        let ciphertext = aes_cipher_string.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher_string.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let key = [0u8; 32];
        let aes_cipher_string = Aes256CipherString::new(key);

        let invalid_base64 = String::from("This is not valid base64!");

        // Attempt to decrypt invalid base64
        let result = aes_cipher_string.decrypt(&invalid_base64);

        // Assert that the result is an error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrypDecStringError::InvalidLength
        ));
    }

    #[test]
    fn test_decrypt_invalid_utf8() {
        let key = [0u8; 32];
        let aes_cipher_string = Aes256CipherString::new(key);

        // Create invalid UTF-8 data by encrypting and then corrupting the result
        let plaintext = String::from("Hello, world!");
        let ciphertext = aes_cipher_string.encrypt(&plaintext).unwrap();
        let mut corrupted_bytes = base64::decode(ciphertext).unwrap();
        corrupted_bytes[0] = 0xff; // Introduce invalid UTF-8
        let corrupted_ciphertext = base64::encode(corrupted_bytes);

        // Attempt to decrypt corrupted ciphertext
        let result = aes_cipher_string.decrypt(&corrupted_ciphertext);

        // Assert that the result is an error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrypDecStringError::Utf8Error(_)
        ));
    }
}
