use aes::Aes256;
use cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use rand::Rng;
use std::fmt;

use super::cryp_dec::CrypDec;

// Define a struct to hold the key
pub struct Aes256Cipher {
    key: [u8; 32],
}

// Define error type for encryption/decryption
#[derive(Debug)]
pub enum CrypDecError {
    InvalidLength,
}

// Implement `std::fmt::Display` for `CrypDecError`
impl fmt::Display for CrypDecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CrypDecError::InvalidLength => write!(f, "Invalid Length"),
        }
    }
}

impl std::error::Error for CrypDecError {}

// Implement the CrypDec trait
impl CrypDec for Aes256Cipher {
    type Input = [u8; 16]; // AES encryption/decryption operates on 16-byte blocks
    type Output = [u8; 16];
    type Error = CrypDecError;

    fn encrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error> {
        let cipher = Aes256::new(GenericArray::from_slice(&self.key));
        let mut block = GenericArray::clone_from_slice(data);
        cipher.encrypt_block(&mut block);
        Ok(block.into())
    }

    fn decrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error> {
        let cipher = Aes256::new(GenericArray::from_slice(&self.key));
        let mut block = GenericArray::clone_from_slice(data);
        cipher.decrypt_block(&mut block);
        Ok(block.into())
    }
}

impl Aes256Cipher {
    pub fn new(key: [u8; 32]) -> Self {
        Aes256Cipher { key }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32]; // Using a zeroed key for simplicity
        let aes_cipher = Aes256Cipher::new(key);

        let plaintext: [u8; 16] = *b"exampleplaintext";

        // Encrypt the plaintext
        let ciphertext = aes_cipher.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);
    }

    #[test]
    fn test_encrypt_decrypt_with_random_key() {
        let key = rand::thread_rng().gen::<[u8; 32]>();
        let aes_cipher = Aes256Cipher::new(key);

        let plaintext: [u8; 16] = *b"exampleplaintext";

        // Encrypt the plaintext
        let ciphertext = aes_cipher.encrypt(&plaintext).unwrap();

        // Decrypt the ciphertext
        let decrypted_text = aes_cipher.decrypt(&ciphertext).unwrap();

        // Assert that the decrypted text matches the original plaintext
        assert_eq!(plaintext, decrypted_text);

        assert_ne!(ciphertext, plaintext)
    }
}
