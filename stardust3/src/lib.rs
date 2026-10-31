//! Stardust combines Astra's transactional file XOR with VersaKey's five
//! deterministic key generators. The two domains have independent file policies.
pub mod app;
pub mod keygen;
pub mod xor;
