mod hash;

mod mnemonic;
pub use mnemonic::{MnemonicBuilder, MnemonicBuilderError};

mod private_key;
pub use private_key::WalletError;

#[cfg(feature = "yubihsm")]
mod yubi;

use crate::{to_eip155_v, Signer, Tron};
use ethers_core::{
    k256::{
        ecdsa::{recoverable::Signature as RecoverableSignature, signature::DigestSigner},
        elliptic_curve::FieldBytes,
        Secp256k1,
    },
    types::{
        transaction::{eip2718::TypedTransaction, eip712::Eip712},
        Address, Signature, H256, U256,
    },
    utils::hash_message,
};
use hash::Sha256Proxy;

use async_trait::async_trait;
use std::fmt;
use base58::ToBase58;
use sha2::{Sha256, Digest};

/// An Ethereum private-public key pair which can be used for signing messages.
///
/// # Examples
///
/// ## Signing and Verifying a message
///
/// The wallet can be used to produce ECDSA [`Signature`] objects, which can be
/// then verified. Note that this uses [`hash_message`] under the hood which will
/// prefix the message being hashed with the `Ethereum Signed Message` domain separator.
///
/// ```
/// use ethers_core::rand::thread_rng;
/// use ethers_signers::{LocalWallet, Signer};
///
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// let wallet = LocalWallet::new(&mut thread_rng());
///
/// // Optionally, the wallet's chain id can be set, in order to use EIP-155
/// // replay protection with different chains
/// let wallet = wallet.with_chain_id(1337u64);
///
/// // The wallet can be used to sign messages
/// let message = b"hello";
/// let signature = wallet.sign_message(message).await?;
/// assert_eq!(signature.recover(&message[..]).unwrap(), wallet.address());
///
/// // LocalWallet is clonable:
/// let wallet_clone = wallet.clone();
/// let signature2 = wallet_clone.sign_message(message).await?;
/// assert_eq!(signature, signature2);
/// # Ok(())
/// # }
/// ```
///
/// [`Signature`]: ethers_core::types::Signature
/// [`hash_message`]: fn@ethers_core::utils::hash_message
#[derive(Clone)]
pub struct Wallet<D: DigestSigner<Sha256Proxy, RecoverableSignature>> {
    /// The Wallet's private Key
    pub(crate) signer: D,
    /// The wallet's address
    pub(crate) address: Address,
    /// The wallet's chain id (for EIP-155)
    pub(crate) chain_id: u64,
}

impl<D: DigestSigner<Sha256Proxy, RecoverableSignature>> Wallet<D> {
    /// Construct a new wallet with an external Signer
    pub fn new_with_signer(signer: D, address: Address, chain_id: u64) -> Self {
        Wallet { signer, address, chain_id }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<D: Sync + Send + DigestSigner<Sha256Proxy, RecoverableSignature>> Signer for Wallet<D> {
    type Error = WalletError;

    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error> {
        let message = message.as_ref();
        let message_hash = hash_message(message);

        Ok(self.sign_hash(message_hash))
    }

    async fn sign_transaction(&self, tx: &TypedTransaction) -> Result<Signature, Self::Error> {
        let mut tx_with_chain = tx.clone();
        if tx_with_chain.chain_id().is_none() {
            // in the case we don't have a chain_id, let's use the signer chain id instead
            tx_with_chain.set_chain_id(self.chain_id);
        }
        Ok(self.sign_transaction_sync(&tx_with_chain))
    }

    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error> {
        let encoded =
            payload.encode_eip712().map_err(|e| Self::Error::Eip712Error(e.to_string()))?;

        Ok(self.sign_hash(H256::from(encoded)))
    }

    fn address(&self) -> Address {
        self.address
    }

    /// Gets the wallet's chain id
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Sets the wallet's chain_id, used in conjunction with EIP-155 signing
    fn with_chain_id<T: Into<u64>>(mut self, chain_id: T) -> Self {
        self.chain_id = chain_id.into();
        self
    }
}

impl<D: DigestSigner<Sha256Proxy, RecoverableSignature>> Wallet<D> {
    /// Synchronously signs the provided transaction, normalizing the signature `v` value with
    /// EIP-155 using the transaction's `chain_id`, or the signer's `chain_id` if the transaction
    /// does not specify one.
    pub fn sign_transaction_sync(&self, tx: &TypedTransaction) -> Signature {
        // rlp (for sighash) must have the same chain id as v in the signature
        let chain_id = tx.chain_id().map(|id| id.as_u64()).unwrap_or(self.chain_id);
        let mut tx = tx.clone();
        tx.set_chain_id(chain_id);

        let sighash = tx.sighash();
        let mut sig = self.sign_hash(sighash);

        // sign_hash sets `v` to recid + 27, so we need to subtract 27 before normalizing
        sig.v = to_eip155_v(sig.v as u8 - 27, chain_id);
        sig
    }

    /// Signs the provided hash.
    pub fn sign_hash(&self, hash: H256) -> Signature {
        let recoverable_sig: RecoverableSignature =
            self.signer.sign_digest(Sha256Proxy::from(hash));

        let v = u8::from(recoverable_sig.recovery_id()) as u64 + 27;

        let r_bytes: FieldBytes<Secp256k1> = recoverable_sig.r().into();
        let s_bytes: FieldBytes<Secp256k1> = recoverable_sig.s().into();
        let r = U256::from_big_endian(r_bytes.as_slice());
        let s = U256::from_big_endian(s_bytes.as_slice());

        Signature { r, s, v }
    }

    /// Gets the wallet's signer
    pub fn signer(&self) -> &D {
        &self.signer
    }
}

// do not log the signer
impl<D: DigestSigner<Sha256Proxy, RecoverableSignature>> fmt::Debug for Wallet<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address)
            .field("chain_Id", &self.chain_id)
            .finish()
    }
}

impl<D: DigestSigner<Sha256Proxy, RecoverableSignature>> Tron for Wallet<D> {
    fn to_tron_hex_address(&self) -> String {
        let mut raw = [0x41; 21];
        raw[1..21].copy_from_slice(self.address.as_ref());
        hex::encode(raw)
    }

    fn to_tron_b58_address(&self) -> String {
        let mut raw = [0x41; 21];
        raw[1..21].copy_from_slice(self.address.as_ref());
        let mut hasher = Sha256::new();
        hasher.update(raw.as_ref());
        let digest1 = hasher.finalize();

        let mut hasher = Sha256::new();
        hasher.update(&digest1);
        let digest = hasher.finalize();

        let mut res = Vec::new();
        res.extend_from_slice(&raw);
        res.extend_from_slice(&digest[..4]);
        res.as_slice().to_base58()
    }
}

mod test{
    use std::str::FromStr;
    use crate::{LocalWallet, Tron};

    #[test]
    fn tron_test() {
        let private_key = "427139B43028A492E2705BCC9C64172392B8DB59F3BA1AEDAE41C88924960091";
        let wallet = LocalWallet::from_str(private_key).unwrap();
        assert_eq!(
            "412A2B9F7641D0750C1E822D0E49EF765C8106524B",
            wallet.to_tron_hex_address().to_uppercase().as_str()
        );
        assert_eq!(
            "TDpBe64DqirkKWj6HWuR1pWgmnhw2wDacE",
            wallet.to_tron_b58_address().as_str()
        );
    }

}