#![no_std]

multiversx_sc::imports!();

pub mod common;

use common::{config::*, consts::*, errors::*};

#[multiversx_sc::contract]
pub trait TFNLaunchpadContract<ContractReader>:
    common::config::ConfigModule
{
    #[init]
    fn init(
        &self,
        main_dao_address: ManagedAddress,
        template_dao_address: ManagedAddress
    ) {
        self.main_dao().set(main_dao_address);
        self.template_dao().set(template_dao_address);
        self.set_state_inactive();
    }

    #[upgrade]
    fn upgrade(&self) {
        self.set_state_inactive();
    }

    #[payable("*")]
    #[endpoint(newLaunchpad)]
    fn new_launchpad(
        &self,
        owner: ManagedAddress,
        kyc_enforced: bool,
        title: ManagedBuffer,
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

        let payment = self.call_value().single_esdt();
        require!(self.token_launchpad_id(&payment.token_identifier).is_empty(), ERROR_TOKEN_ALREADY_LAUNCHED);
        require!(payment.amount > 0, ERROR_ZERO_PAYMENT);

        let new_id = self.last_launchpad_id().get() + 1;
        let launchpad = Launchpad{
            id: new_id,
            owner,
            kyc_enforced,
            title,
            token: payment.token_identifier,
            amount: payment.amount,
            payment_token,
            price,
            min_buy_amount,
            max_buy_amount,
            start_time,
            end_time,
            total_raised: BigUint::zero(),
            total_sold: BigUint::zero(),
            redeemed: false
        };
        self.last_launchpad_id().set(new_id);
        self.launchpads(new_id).set(launchpad);

        new_id
    }

    #[payable("*")]
    #[endpoint]
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

    #[endpoint]
    fn redeem(&self, id: u64) {
        require!(self.state().get() == State::Active, ERROR_NOT_ACTIVE);

        let mut launchpad = self.launchpads(id).get();
        require!(launchpad.end_time < self.blockchain().get_block_timestamp(), ERROR_LAUNCHPAD_NOT_ENDED);
        require!(!launchpad.redeemed, ERROR_ALREADY_REDEEMED);

        if launchpad.total_raised > 0 {
            self.send().direct_esdt(
                &launchpad.owner,
                &launchpad.payment_token,
                0,
                &launchpad.total_raised
            );
        }

        let left_amount = &launchpad.amount - &launchpad.total_sold;
        if left_amount > 0 {
            self.send().direct_esdt(
                &launchpad.owner,
                &launchpad.token,
                0,
                &left_amount
            );
        }

        launchpad.redeemed = true;
        self.launchpads(id).set(launchpad);
    }
}
