#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, String, Symbol,
};

/// Storage keys for contract state
#[contracttype]
pub enum DataKey {
    Admin,
    ExchangeRate,    // How many local tokens per 1 XLM (scaled by RATE_PRECISION)
    LocalToken,      // Address of the local currency token contract
    XlmToken,        // Address of the XLM token (wrapped)
    FeePercent,      // Fee in basis points (e.g. 30 = 0.3%)
    Paused,
    TotalSwapped,    // Lifetime XLM swapped (for analytics)
    CurrencyLabel,   // e.g. "INR", "NGN", "BRL"
}

/// Events emitted by the contract
#[contracttype]
pub struct SwapEvent {
    pub user: Address,
    pub xlm_in: i128,
    pub local_out: i128,
    pub rate: i128,
    pub fee: i128,
}

/// Precision factor for exchange rate (6 decimals)
const RATE_PRECISION: i128 = 1_000_000;

/// Maximum fee: 5% = 500 basis points
const MAX_FEE_BPS: i128 = 500;

#[contract]
pub struct XlmLocalSwap;

#[contractimpl]
impl XlmLocalSwap {
    // ─────────────────────────────────────────────
    // ADMIN: Initialize the contract
    // ─────────────────────────────────────────────

    /// Initialize the swap contract.
    ///
    /// # Arguments
    /// - `admin`         – Address that controls rates and settings
    /// - `xlm_token`     – Wrapped XLM token contract address
    /// - `local_token`   – Local currency token contract address
    /// - `rate`          – Exchange rate: local tokens per XLM × RATE_PRECISION
    ///                     e.g. for 1 XLM = 83.5 INR → rate = 83_500_000
    /// - `fee_bps`       – Fee in basis points (30 = 0.30%)
    /// - `currency_label`– Human-readable symbol, e.g. "INR"
    pub fn initialize(
        env: Env,
        admin: Address,
        xlm_token: Address,
        local_token: Address,
        rate: i128,
        fee_bps: i128,
        currency_label: String,
    ) {
        // Prevent re-initialization
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("contract already initialized");
        }

        assert!(rate > 0, "rate must be positive");
        assert!(fee_bps >= 0 && fee_bps <= MAX_FEE_BPS, "fee out of range");

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::XlmToken, &xlm_token);
        env.storage().instance().set(&DataKey::LocalToken, &local_token);
        env.storage().instance().set(&DataKey::ExchangeRate, &rate);
        env.storage().instance().set(&DataKey::FeePercent, &fee_bps);
        env.storage().instance().set(&DataKey::CurrencyLabel, &currency_label);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::TotalSwapped, &0i128);
    }

    // ─────────────────────────────────────────────
    // CORE: Swap XLM → Local token
    // ─────────────────────────────────────────────

    /// Swap XLM for local currency tokens.
    ///
    /// The user must have approved this contract to transfer `xlm_amount`
    /// from their account before calling this function.
    ///
    /// # Arguments
    /// - `user`       – Caller / recipient of local tokens
    /// - `xlm_amount` – Amount of XLM (in stroops, 1 XLM = 10_000_000 stroops)
    /// - `min_out`    – Minimum local tokens expected (slippage protection)
    pub fn swap_xlm_to_local(
        env: Env,
        user: Address,
        xlm_amount: i128,
        min_out: i128,
    ) -> i128 {
        user.require_auth();
        Self::assert_not_paused(&env);

        assert!(xlm_amount > 0, "xlm_amount must be positive");

        let rate: i128 = env.storage().instance().get(&DataKey::ExchangeRate).unwrap();
        let fee_bps: i128 = env.storage().instance().get(&DataKey::FeePercent).unwrap();
        let xlm_token: Address = env.storage().instance().get(&DataKey::XlmToken).unwrap();
        let local_token: Address = env.storage().instance().get(&DataKey::LocalToken).unwrap();

        // Calculate gross local tokens
        let gross_local = xlm_amount
            .checked_mul(rate)
            .expect("overflow in rate calc")
            / RATE_PRECISION;

        // Deduct fee
        let fee_amount = gross_local
            .checked_mul(fee_bps)
            .expect("overflow in fee calc")
            / 10_000;
        let net_local = gross_local - fee_amount;

        assert!(net_local >= min_out, "slippage: output below minimum");
        assert!(net_local > 0, "output too small");

        // Transfer XLM from user → contract
        let xlm_client = token::Client::new(&env, &xlm_token);
        xlm_client.transfer(&user, &env.current_contract_address(), &xlm_amount);

        // Transfer local tokens from contract → user
        let local_client = token::Client::new(&env, &local_token);
        local_client.transfer(&env.current_contract_address(), &user, &net_local);

        // Update lifetime counter
        let prev: i128 = env.storage().instance().get(&DataKey::TotalSwapped).unwrap_or(0);
        env.storage().instance().set(&DataKey::TotalSwapped, &(prev + xlm_amount));

        // Emit event
        env.events().publish(
            (Symbol::new(&env, "swap"),),
            SwapEvent {
                user,
                xlm_in: xlm_amount,
                local_out: net_local,
                rate,
                fee: fee_amount,
            },
        );

        net_local
    }

    // ─────────────────────────────────────────────
    // VIEW: Quote before swapping
    // ─────────────────────────────────────────────

    /// Returns `(gross_local, fee_amount, net_local)` for a given XLM input.
    /// Call this off-chain before submitting a swap.
    pub fn quote(env: Env, xlm_amount: i128) -> (i128, i128, i128) {
        let rate: i128 = env.storage().instance().get(&DataKey::ExchangeRate).unwrap();
        let fee_bps: i128 = env.storage().instance().get(&DataKey::FeePercent).unwrap();

        let gross = xlm_amount * rate / RATE_PRECISION;
        let fee = gross * fee_bps / 10_000;
        let net = gross - fee;
        (gross, fee, net)
    }

    /// Return the current exchange rate (scaled by RATE_PRECISION).
    pub fn get_rate(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::ExchangeRate).unwrap()
    }

    /// Return fee in basis points.
    pub fn get_fee(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::FeePercent).unwrap()
    }

    /// Lifetime XLM swapped through this contract.
    pub fn total_swapped(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalSwapped).unwrap_or(0)
    }

    /// Return the local currency label (e.g. "INR").
    pub fn currency_label(env: Env) -> String {
        env.storage().instance().get(&DataKey::CurrencyLabel).unwrap()
    }

    /// Is the contract paused?
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    // ─────────────────────────────────────────────
    // ADMIN: Manage contract settings
    // ─────────────────────────────────────────────

    /// Update the exchange rate. Only callable by admin.
    pub fn set_rate(env: Env, new_rate: i128) {
        Self::require_admin(&env);
        assert!(new_rate > 0, "rate must be positive");
        env.storage().instance().set(&DataKey::ExchangeRate, &new_rate);
        env.events().publish((Symbol::new(&env, "rate_updated"),), new_rate);
    }

    /// Update the fee. Only callable by admin.
    pub fn set_fee(env: Env, new_fee_bps: i128) {
        Self::require_admin(&env);
        assert!(new_fee_bps >= 0 && new_fee_bps <= MAX_FEE_BPS, "fee out of range");
        env.storage().instance().set(&DataKey::FeePercent, &new_fee_bps);
    }

    /// Pause or unpause swapping. Only callable by admin.
    pub fn set_paused(env: Env, paused: bool) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &paused);
        env.events().publish((Symbol::new(&env, "paused_changed"),), paused);
    }

    /// Withdraw accumulated XLM fees to admin wallet. Only callable by admin.
    pub fn withdraw_xlm(env: Env, amount: i128) {
        Self::require_admin(&env);
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        let xlm_token: Address = env.storage().instance().get(&DataKey::XlmToken).unwrap();
        let xlm_client = token::Client::new(&env, &xlm_token);
        xlm_client.transfer(&env.current_contract_address(), &admin, &amount);
    }

    /// Transfer admin rights to a new address.
    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    // ─────────────────────────────────────────────
    // INTERNAL HELPERS
    // ─────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn assert_not_paused(env: &Env) {
        let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
        assert!(!paused, "contract is paused");
    }
}

// ─────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────
#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        token::{Client as TokenClient, StellarAssetClient},
        vec, Address, Env, String,
    };

    fn create_token(env: &Env, admin: &Address) -> (Address, StellarAssetClient) {
        let contract_id = env.register_stellar_asset_contract(admin.clone());
        let client = StellarAssetClient::new(env, &contract_id);
        (contract_id, client)
    }

    fn setup() -> (Env, Address, Address, Address, Address, XlmLocalSwapClient) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        // Create mock tokens
        let (xlm_addr, xlm_admin) = create_token(&env, &admin);
        let (local_addr, local_admin) = create_token(&env, &admin);

        // Mint tokens to user (XLM) and contract (local)
        xlm_admin.mint(&user, &100_000_0000000); // 100,000 XLM in stroops
        
        // Deploy swap contract
        let contract_id = env.register_contract(None, XlmLocalSwap);
        let client = XlmLocalSwapClient::new(&env, &contract_id);

        // 1 XLM = 83.5 INR → rate = 83_500_000 (6 decimal precision)
        client.initialize(
            &admin,
            &xlm_addr,
            &local_addr,
            &83_500_000i128,
            &30i128,  // 0.30% fee
            &String::from_str(&env, "INR"),
        );

        // Fund contract with local tokens
        local_admin.mint(&contract_id, &10_000_000_0000000i128);

        (env, admin, user, xlm_addr, local_addr, client)
    }

    #[test]
    fn test_quote() {
        let (env, _admin, _user, _xlm, _local, client) = setup();
        // 10 XLM (in stroops = 10 * 10_000_000 = 100_000_000)
        let (gross, fee, net) = client.quote(&100_000_000i128);
        // gross = 100_000_000 * 83_500_000 / 1_000_000 = 8_350_000_000 (835 INR scaled)
        assert!(gross > 0);
        assert!(net < gross);
        assert_eq!(fee + net, gross);
    }

    #[test]
    fn test_swap() {
        let (env, _admin, user, _xlm, _local, client) = setup();
        let xlm_in = 10_000_0000i128; // 10 XLM in stroops
        let net = client.swap_xlm_to_local(&user, &xlm_in, &1i128);
        assert!(net > 0);
    }

    #[test]
    #[should_panic(expected = "slippage")]
    fn test_slippage_protection() {
        let (env, _admin, user, _xlm, _local, client) = setup();
        // Set impossibly high min_out to trigger slippage guard
        client.swap_xlm_to_local(&user, &10_000_0000i128, &i128::MAX);
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_pause() {
        let (env, admin, user, _xlm, _local, client) = setup();
        client.set_paused(&true);
        client.swap_xlm_to_local(&user, &10_000_0000i128, &1i128);
    }

    #[test]
    fn test_rate_update() {
        let (env, _admin, _user, _xlm, _local, client) = setup();
        client.set_rate(&90_000_000i128); // 1 XLM = 90 INR
        assert_eq!(client.get_rate(), 90_000_000i128);
    }
}