#![no_std]

multiversx_sc::imports!();

pub mod common;

use common::{config::*, consts::*, errors::*};
use tfn_franchise_dao::ProxyTrait as franchise_dao_proxy;
use tfn_dao::common::config::ProxyTrait as dao_proxy;
use tfn_dex::ProxyTrait as dex_proxy;
use tfn_platform::ProxyTrait as platform_proxy;

#[multiversx_sc::contract]
pub trait TFNLaunchpadContract<ContractReader>:
    common::config::ConfigModule
{
    #[init]
    fn init(
        &self,
        main_dao_address: ManagedAddress,
        dex_address: ManagedAddress,
        platform: ManagedAddress,
        template_dao_address: ManagedAddress,
        template_employee_address: ManagedAddress,
        template_student_address: ManagedAddress,
    ) {
        self.main_dao().set(main_dao_address);
        let governance_token: TokenIdentifier = self.main_dao_contract_proxy()
            .contract(self.main_dao().get())
            .governance_token()
            .execute_on_dest_context();
        self.governance_token().set(governance_token);
        self.dex().set(dex_address);
        self.platform().set(platform);
        self.template_dao().set(template_dao_address);
        self.template_employee().set(template_employee_address);
        self.template_student().set(template_student_address);
        self.set_state_active();
    }

    #[upgrade]
    fn upgrade(&self) {
    }

    #[endpoint(newLaunchpad)]
    fn new_launchpad(
        &self,
        owner: ManagedAddress,
        kyc_enforced: bool,
        description: ManagedBuffer,
        token: TokenIdentifier,
        payment_token: TokenIdentifier,
        price: BigUint, // if payment token is USDC (6 decimals), price should be x_000_000
        min_buy_amount: BigUint,
        max_buy_amount: BigUint,
        start_time: u64,
        end_time: u64
    ) -> u64 {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        self.only_dao();

        require!(price > 0, ERROR_ZERO_PRICE);
        require!(min_buy_amount <= max_buy_amount, ERROR_WRONG_MIN_MAX_AMOUNTS);

        let now = self.blockchain().get_block_timestamp();
        require!(now < start_time, ERROR_WRONG_START_TIME);
        require!(start_time < end_time, ERROR_WRONG_END_TIME);

        require!(self.token_launchpad_id(&token).is_empty(), ERROR_TOKEN_ALREADY_LAUNCHED);

        let launchpad = Launchpad{
            id: self.last_launchpad_id().get(),
            owner,
            kyc_enforced,
            description,
            token: token.clone(),
            amount: BigUint::zero(),
            payment_token,
            price,
            min_buy_amount,
            max_buy_amount,
            start_time,
            end_time,
            total_raised: BigUint::zero(),
            total_sold: BigUint::zero(),
            deployed: false,
            status: Status::Pending,
        };
        self.launchpads(launchpad.id).set(&launchpad);
        self.token_launchpad_id(&token).set(launchpad.id);
        self.last_launchpad_id().set(launchpad.id + 1);

        launchpad.id
    }

    #[payable("*")]
    #[endpoint(addTokens)]
    fn add_tokens(&self, id: u64) {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        require!(!self.launchpads(id).is_empty(), ERROR_LAUNCHPAD_NOT_FOUND);

        let mut launchpad = self.launchpads(id).get();
        require!(launchpad.end_time > self.blockchain().get_block_timestamp(), ERROR_LAUNCHPAD_INACTIVE);

        let payment = self.call_value().single_esdt();
        require!(launchpad.token == payment.token_identifier, ERROR_WRONG_TOKEN);

        launchpad.amount += payment.amount;
        self.launchpads(id).set(launchpad);
    }

    #[endpoint(cancelLaunchpad)]
    fn cancel_launchpad(&self, id: u64) {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        self.only_launchpad_owner(id);

        let launchpad = self.launchpads(id).get();
        require!(launchpad.total_sold == 0, ERROR_DELETING_LAUNCHPAD);

        self.launchpads(id).clear();
        self.token_launchpad_id(&launchpad.token).clear();

        if launchpad.amount > 0 {
            self.send().direct_esdt(
                &launchpad.owner,
                &launchpad.token,
                0,
                &launchpad.amount
            );
        }
        if self.last_launchpad_id().get() ==  id + 1 {
            self.last_launchpad_id().set(id);
        }
    }

    #[endpoint(whitelistUser)]
    fn whitelist_user(&self, id: u64, user: ManagedAddress) {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        self.only_launchpad_owner(id);

        self.whitelisted_users(id).insert(user);
    }

    #[payable("*")]
    #[endpoint(buy)]
    fn buy(&self, id: u64) {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        require!(!self.launchpads(id).is_empty(), ERROR_LAUNCHPAD_NOT_FOUND);

        let mut launchpad = self.launchpads(id).get();
        require!(launchpad.is_active(self.blockchain().get_block_timestamp()), ERROR_LAUNCHPAD_INACTIVE);

        let payment = self.call_value().single_esdt();
        require!(payment.token_identifier == launchpad.payment_token, ERROR_WRONG_TOKEN);

        let token_amount = &payment.amount * ONE / &launchpad.price;
        let caller = self.blockchain().get_caller();
        require!(!launchpad.kyc_enforced || self.whitelisted_users(id).contains(&caller), ERROR_NOT_WHITELISTED);

        let old_bought_amount = self.user_participation(&caller, id).get();
        require!(
            &token_amount + &old_bought_amount >= launchpad.min_buy_amount,
            ERROR_LOW_AMOUNT
        );
        require!(
            &token_amount + &old_bought_amount <= launchpad.max_buy_amount,
            ERROR_HIGH_AMOUNT
        );
        require!(
            &token_amount + &launchpad.total_sold <= launchpad.amount,
            ERROR_INSUFFICIENT_FUNDS
        );

        self.send().direct_esdt(
            &caller,
            &launchpad.token,
            0,
            &token_amount
        );

        launchpad.total_raised += payment.amount;
        launchpad.total_sold += &token_amount;
        self.launchpads(id).set(launchpad);

        self.user_participation(&caller, id).update(|value| *value += &token_amount);
        self.user_launchpads(&caller).insert(id);
        self.launchpad_users(id).insert(caller);
    }

    #[payable("EGLD")]
    #[endpoint(deployFranchise)]
    fn deploy_franchise(&self, id: u64) -> ManagedAddress {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);
        require!(!self.launchpads(id).is_empty(), ERROR_LAUNCHPAD_NOT_FOUND);

        let mut launchpad = self.launchpads(id).get();
        require!(launchpad.end_time < self.blockchain().get_block_timestamp(), ERROR_LAUNCHPAD_NOT_ENDED);
        require!(!launchpad.deployed, ERROR_ALREADY_DEPLOYED);

        let (new_address, ()) = self
            .franchise_dao_contract_proxy()
            .init(
                &launchpad.owner,
                &launchpad.token,
            )
            .deploy_from_source(
                &self.template_dao().get(),
                CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE | CodeMetadata::PAYABLE_BY_SC,
            );

        let mut payments: ManagedVec<EsdtTokenPayment> = ManagedVec::new();
        if launchpad.total_raised > 0 {
            payments.push(EsdtTokenPayment::new(launchpad.payment_token.clone(), 0, launchpad.total_raised.clone()));
        }

        let left_amount = &launchpad.amount - &launchpad.total_sold;
        if left_amount > 0 {
            payments.push(EsdtTokenPayment::new(launchpad.token.clone(), 0, left_amount.clone()));
        }

        if !payments.is_empty() {
            self.franchise_dao_contract_proxy()
                .contract(new_address.clone())
                .add_funds()
                .multi_esdt(payments)
                .execute_on_dest_context::<()>();
        }

        self.main_dao_contract_proxy()
            .contract(self.main_dao().get())
            .franchise_deployed(new_address.clone())
            .execute_on_dest_context::<()>();

        self.platform_contract_proxy()
            .contract(self.platform().get())
            .subscribe_franchise(new_address.clone())
            .execute_on_dest_context::<()>();

        self.dex_contract_proxy()
            .contract(self.dex().get())
            .create_pair(self.governance_token().get(), &launchpad.token, 18)
            .with_egld_transfer(self.call_value().egld_value().clone_value())
            .gas(GAS_LIMIT_FOR_CREATE_PAIR)
            .execute_on_dest_context::<()>();

        launchpad.deployed = true;
        self.deployed_launchpads(new_address.clone()).set(id);
        self.launchpads(id).set(launchpad);

        new_address
    }

    #[endpoint(upgradeFranchise)]
    fn upgrade_franchise(&self, franchise_address: ManagedAddress, args: OptionalValue<ManagedArgBuffer<Self::Api>>) {
        let caller = self.blockchain().get_caller();
        require!(caller == self.blockchain().get_owner_address() || caller == self.main_dao().get(), ERROR_ONLY_OWNER_OR_DAO);

        let upgrade_args = match args {
            OptionalValue::Some(args) => args,
            OptionalValue::None => ManagedArgBuffer::new(),            
        };
        let gas_left = self.blockchain().get_gas_left();
        self.tx()
            .to(franchise_address)
            .gas(gas_left)
            .raw_upgrade()
            .arguments_raw(upgrade_args)
            .from_source(self.template_dao().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE | CodeMetadata::PAYABLE_BY_SC)
            .upgrade_async_call_and_exit();
    }

    // proxies
    #[proxy]
    fn main_dao_contract_proxy(&self) -> tfn_dao::Proxy<Self::Api>;

    #[proxy]
    fn dex_contract_proxy(&self) -> tfn_dex::Proxy<Self::Api>;

    #[proxy]
    fn franchise_dao_contract_proxy(&self) -> tfn_franchise_dao::Proxy<Self::Api>;

    #[proxy]
    fn platform_contract_proxy(&self) -> tfn_platform::Proxy<Self::Api>;
}
