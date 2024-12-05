multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::common::errors::*;

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Copy, Clone, Debug)]
pub enum State {
    Inactive,
    Active,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Clone, Debug)]
pub struct Launchpad<M: ManagedTypeApi> {
    pub id: u64,
    pub owner: ManagedAddress<M>,
    pub kyc_enforced: bool,
    pub title: ManagedBuffer<M>,
    pub token: TokenIdentifier<M>, // should have 18 decimals. please check in front end
    pub amount: BigUint<M>,
    pub payment_token: TokenIdentifier<M>,
    pub price: BigUint<M>, // if payment token is USDC (6 decimals), price should be x_000_000
    pub min_buy_amount: BigUint<M>,
    pub max_buy_amount: BigUint<M>,
    pub start_time: u64,
    pub end_time: u64,
    pub total_raised: BigUint<M>,
    pub total_sold: BigUint<M>,
    pub redeemed: bool,
}

impl<M> Launchpad<M>
where M: ManagedTypeApi {
    pub fn is_active(&self, current_timestamp: u64) -> bool {
        return current_timestamp >= self.start_time && current_timestamp <= self.end_time && self.total_sold < self.amount
    }
}

#[multiversx_sc::module]
pub trait ConfigModule {
    // state
    #[only_owner]
    #[endpoint(setStateActive)]
    fn set_state_active(&self) {
        self.state().set(State::Active);
    }

    #[only_owner]
    #[endpoint(setStateInactive)]
    fn set_state_inactive(&self) {
        self.state().set(State::Inactive);
    }

    #[view(getState)]
    #[storage_mapper("state")]
    fn state(&self) -> SingleValueMapper<State>;

    // main dao sc address
    #[view(getMainDAO)]
    #[storage_mapper("main_dao")]
    fn main_dao(&self) -> SingleValueMapper<ManagedAddress>;

    // template dao sc address
    #[view(getTemplateDAO)]
    #[storage_mapper("template_dao")]
    fn template_dao(&self) -> SingleValueMapper<ManagedAddress>;

    // launchpads
    #[view(getLaunchpads)]
    #[storage_mapper("launchpads")]
    fn launchpads(&self, id: u64) -> SingleValueMapper<Launchpad<Self::Api>>;

    #[view(getLastLaunchpadId)]
    #[storage_mapper("last_launchpad_id")]
    fn last_launchpad_id(&self) -> SingleValueMapper<u64>;

    #[view(getLaunchpadIdByToken)]
    #[storage_mapper("token_launchpad_id")]
    fn token_launchpad_id(&self, token: &TokenIdentifier) -> SingleValueMapper<u64>;

    #[view(getLaunchpadUsers)]
    #[storage_mapper("launchpad_users")]
    fn launchpad_users(&self, id: u64) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getUserLaunchpads)]
    #[storage_mapper("user_launchpads")]
    fn user_launchpads(&self, user: &ManagedAddress) -> UnorderedSetMapper<u64>;

    #[view(getUserParticipation)]
    #[storage_mapper("user_participation")]
    fn user_participation(&self, user: &ManagedAddress, id: u64) -> SingleValueMapper<BigUint>;

    // kyc
    #[view(getWhitelistedUsers)]
    #[storage_mapper("whitelisted_users")]
    fn whitelisted_users(&self, id: u64) -> UnorderedSetMapper<ManagedAddress>;

    #[endpoint(whitelistUser)]
    fn whitelist_user(&self, id: u64, user: ManagedAddress) {
        self.only_launchpad_owner(id);

        self.whitelisted_users(id).insert(user);
    }

    // helpers
    fn only_dao(&self) {
        require!(self.blockchain().get_caller() == self.main_dao().get(), ERROR_ONLY_MAIN_DAO);
    }

    fn only_launchpad_owner(&self, id: u64) {
        require!(!self.launchpads(id).is_empty(), ERROR_LAUNCHPAD_NOT_FOUND);

        let launchpad = self.launchpads(id).get();
        require!(self.blockchain().get_caller() == launchpad.owner, ERROR_ONLY_LAUNCHPAD_OWNER);
    }
}
