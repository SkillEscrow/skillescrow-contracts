#![no_std]

use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct BountyEscrow;

#[contractimpl]
impl BountyEscrow {}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn contract_registers() {
        let env = Env::default();
        let _id = env.register(BountyEscrow, ());
    }
}
