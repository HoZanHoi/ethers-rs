//! Helpers for creating wallets for Tron
pub trait Tron {
    fn to_tron_hex_address(&self) -> String;
    fn to_tron_b58_address(&self) -> String;
}