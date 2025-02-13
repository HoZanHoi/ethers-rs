//! Provides a unified interface for locally signing transactions.
#![deny(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]

mod wallet;
pub use wallet::{MnemonicBuilder, Wallet, WalletError};

/// Re-export the BIP-32 crate so that wordlists can be accessed conveniently.
pub use coins_bip39;

/// A wallet instantiated with a locally stored private key
pub type LocalWallet = Wallet<ethers_core::k256::ecdsa::SigningKey>;

#[cfg(feature = "yubi")]
/// A wallet instantiated with a YubiHSM
pub type YubiWallet = Wallet<yubihsm::ecdsa::Signer<ethers_core::k256::Secp256k1>>;

#[cfg(feature = "ledger")]
mod ledger;
#[cfg(feature = "ledger")]
pub use ledger::{
    app::LedgerEthereum as Ledger,
    types::{DerivationType as HDPath, LedgerError},
};

#[cfg(feature = "trezor")]
mod trezor;
#[cfg(feature = "trezor")]
pub use trezor::{
    app::TrezorEthereum as Trezor,
    types::{DerivationType as TrezorHDPath, TrezorError},
};

#[cfg(feature = "yubi")]
pub use yubihsm;

#[cfg(feature = "aws")]
mod aws;

#[cfg(feature = "aws")]
pub use aws::{AwsSigner, AwsSignerError};

use async_trait::async_trait;
use ethers_core::types::{
    transaction::{eip2718::TypedTransaction, eip712::Eip712},
    Address, Signature,
};
use std::error::Error;

/// Applies [EIP155](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-155.md)
pub fn to_eip155_v<T: Into<u8>>(recovery_id: T, chain_id: u64) -> u64 {
    (recovery_id.into() as u64) + 35 + chain_id * 2
}

/// Trait for signing transactions and messages
///
/// Implement this trait to support different signing modes, e.g. Ledger, hosted etc.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Signer: std::fmt::Debug + Send + Sync {
    type Error: Error + Send + Sync;
    /// Signs the hash of the provided message after prefixing it
    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error>;

    /// Signs the transaction
    async fn sign_transaction(&self, message: &TypedTransaction) -> Result<Signature, Self::Error>;

    /// Encodes and signs the typed data according EIP-712.
    /// Payload must implement Eip712 trait.
    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error>;

    /// Returns the signer's Ethereum Address
    fn address(&self) -> Address;

    /// Returns the signer's chain id
    fn chain_id(&self) -> u64;

    /// Sets the signer's chain id
    #[must_use]
    fn with_chain_id<T: Into<u64>>(self, chain_id: T) -> Self;
}

pub trait Tron {
    /// Returns the signer's Tron Address (Hex format)
    fn to_tron_hex_address(&self) -> String;
    /// Returns the signer's Tron Address (base58 format)
    fn to_tron_b58_address(&self) -> String;
}