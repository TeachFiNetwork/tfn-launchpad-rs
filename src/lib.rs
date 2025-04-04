#![no_std]

multiversx_sc::imports!();

pub mod common;

use common::{config::*, consts::*, errors::*};
use tfn_franchise_dao::{ProxyTrait as franchise_dao_proxy, common::config::ProxyTrait as _};
use tfn_dao::common::config::ProxyTrait as dao_proxy;
use tfn_dex::ProxyTrait as dex_proxy;
use tfn_platform::ProxyTrait as platform_proxy;
use tfn_digital_identity::common::config::Identity;

#[multiversx_sc::contract]
pub trait TFNLaunchpadContract<ContractReader>:
    common::config::ConfigModule
{
    #[init]
    fn init(&self) {
    }

    #[upgrade]
    fn upgrade(&self) {
        // self.clear_storage();
    }

    // DEBUG ENDPOINT
    #[only_owner]
    #[endpoint(clearStorage)]
    fn clear_storage(&self) {
        for launchpad_id in 0..self.last_launchpad_id().take() {
            if !self.launchpads(launchpad_id).is_empty() {
                let launchpad = self.launchpads(launchpad_id).get();
                self.token_launchpad_id(&launchpad.token).clear();
                self.whitelisted_users(launchpad_id).clear();
                for user in self.launchpad_users(launchpad_id).iter() {
                    self.user_participation(&user, launchpad_id).clear();
                    self.user_launchpads(&user).clear();
                }
                self.launchpad_users(launchpad_id).clear();
                self.launchpads(launchpad_id).clear();
            }
        }
        // clear deployed_launchpads ?
        self.set_state_inactive();
    }

    #[endpoint(newLaunchpad)]
    fn new_launchpad(
        &self,
        owner: ManagedAddress,
        details: Identity<Self::Api>,
        kyc_enforced: bool,
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
            details,
            kyc_enforced,
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
        self.whitelisted_users(id).clear();

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

        let main_dao_address = self.main_dao().get();
        let template_dao = self.dao_contract_proxy()
            .contract(main_dao_address.clone())
            .template_franchise_dao()
            .execute_on_dest_context();
        let (new_address, ()) = self
            .franchise_dao_contract_proxy()
            .init(
                &launchpad.owner,
                &launchpad.token,
            )
            .deploy_from_source(
                &template_dao,
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

        self.dao_contract_proxy()
            .contract(main_dao_address.clone())
            .franchise_deployed(new_address.clone())
            .execute_on_dest_context::<()>();

        let identity_id = self.digital_identity_contract_proxy()
            .contract(self.digital_identity().get())
            .new_identity(
                launchpad.clone().details.is_corporate,
                launchpad.clone().details.legal_id,
                launchpad.clone().details.birthdate,
                new_address.clone(),
                launchpad.clone().details.name,
                launchpad.clone().details.description,
                launchpad.clone().details.image,
                launchpad.clone().details.contact,
            )
            .execute_on_dest_context::<u64>();

        let platform_address: ManagedAddress = if self.platform().is_empty() {
            let address = self.dao_contract_proxy()
                .contract(main_dao_address)
                .platform_sc()
                .execute_on_dest_context();
            self.platform().set(&address);

            address
        } else {
            self.platform().get()
        };

        self.franchise_dao_contract_proxy()
            .contract(new_address.clone())
            .set_identity_id(identity_id)
            .execute_on_dest_context::<()>();

        self.platform_contract_proxy()
            .contract(platform_address)
            .subscribe_franchise(new_address.clone(), identity_id)
            .execute_on_dest_context::<()>();

        self.dex_contract_proxy()
            .contract(self.dex_sc().get())
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
        let template_dao: ManagedAddress = self.dao_contract_proxy()
            .contract(self.main_dao().get())
            .template_franchise_dao()
            .execute_on_dest_context();
        let gas_left = self.blockchain().get_gas_left();
        self.tx()
            .to(franchise_address)
            .gas(gas_left)
            .raw_upgrade()
            .arguments_raw(upgrade_args)
            .from_source(template_dao)
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE | CodeMetadata::PAYABLE_BY_SC)
            .upgrade_async_call_and_exit();
    }

    // proxies
    #[proxy]
    fn franchise_dao_contract_proxy(&self) -> tfn_franchise_dao::Proxy<Self::Api>;

    #[proxy]
    fn platform_contract_proxy(&self) -> tfn_platform::Proxy<Self::Api>;

    #[proxy]
    fn digital_identity_contract_proxy(&self) -> tfn_digital_identity::Proxy<Self::Api>;
}
