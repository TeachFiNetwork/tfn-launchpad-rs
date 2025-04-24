multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::common::errors::*;
use tfn_dao::common::config::ProxyTrait as _;
use tfn_dex::common::config::ProxyTrait as _;
use tfn_digital_identity::common::config::Identity;

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Copy, Clone, Debug)]
pub enum State {
    Inactive,
    Active,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Copy, Clone, Debug)]
pub enum Status {
    Pending,
    Active,
    Ended,
    Deployed,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Clone, Debug)]
pub struct Launchpad<M: ManagedTypeApi> {
    pub id: u64,
    pub owner: ManagedAddress<M>,
    pub details: Identity<M>,
    pub kyc_enforced: bool,
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
    pub deployed: bool,
    pub status: Status,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Clone, Debug)]
pub struct LaunchpadView<M: ManagedTypeApi> {
    pub bought: BigUint<M>,
    pub whitelisted: bool,
    pub launchpad: Launchpad<M>,
}

impl<M> Launchpad<M>
where M: ManagedTypeApi {
    pub fn is_active(&self, current_timestamp: u64) -> bool {
        current_timestamp >= self.start_time && current_timestamp <= self.end_time && self.total_sold < self.amount
    }

    pub fn get_status(&self, current_timestamp: u64) -> Status {
        if self.start_time <= current_timestamp && self.end_time >= current_timestamp {
            Status::Active
        } else if self.end_time < current_timestamp {
            if self.deployed {
                Status::Deployed
            } else {
                Status::Ended
            }
        } else {
            Status::Pending
        }
    }
}

#[multiversx_sc::module]
pub trait ConfigModule {
    // state
    #[only_owner]
    #[endpoint(setStateActive)]
    fn set_state_active(&self) {
        require!(!self.main_dao().is_empty(), ERROR_DAO_NOT_SET);
        require!(!self.dex_sc().is_empty(), ERROR_DEX_NOT_SET);
        require!(!self.digital_identity().is_empty(), ERROR_DIGITAL_IDENTITY_NOT_SET);

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

    // platform sc address
    #[view(getPlatform)]
    #[storage_mapper("platform")]
    fn platform(&self) -> SingleValueMapper<ManagedAddress>;

    // should be called only by the DAO SC at initialization
    #[endpoint(setMainDAO)]
    fn set_main_dao(&self) {
        require!(self.main_dao().is_empty(), ERROR_DAO_ALREADY_SET);

        let caller = self.blockchain().get_caller();
        self.main_dao().set(&caller);
        let governance_token: TokenIdentifier = self.dao_contract_proxy()
            .contract(caller)
            .governance_token()
            .execute_on_dest_context();
        self.governance_token().set(governance_token);
    }

    // digital identity sc address
    #[view(getDigitalIdentityAddress)]
    #[storage_mapper("digital_identity")]
    fn digital_identity(&self) -> SingleValueMapper<ManagedAddress>;

    // should be called only by the DAO SC at initialization
    #[endpoint(setDigitalIdentity)]
    fn set_digital_identity(&self, address: ManagedAddress) {
        if !self.digital_identity().is_empty() {
            return
        }

        self.digital_identity().set(address);
    }

    #[view(getGovernanceToken)]
    #[storage_mapper("governance_token")]
    fn governance_token(&self) -> SingleValueMapper<TokenIdentifier>;

    // dex sc address
    #[view(getDEX)]
    #[storage_mapper("dex_sc")]
    fn dex_sc(&self) -> SingleValueMapper<ManagedAddress>;

    #[endpoint(setDEX)]
    fn set_dex(&self, address: ManagedAddress) {
        require!(self.dex_sc().is_empty(), ERROR_DEX_ALREADY_SET);

        self.dex_sc().set(&address);
        self.dex_contract_proxy()
            .contract(address)
            .set_launchpad_address()
            .execute_on_dest_context::<()>();
    }

    // launchpads
    #[view(getLaunchpad)]
    #[storage_mapper("launchpads")]
    fn launchpads(&self, id: u64) -> SingleValueMapper<Launchpad<Self::Api>>;

    #[view(getAllLaunchpads)]
    fn get_all_launchpads(
        &self,
        start_idx: u64,
        end_idx: u64,
        address: ManagedAddress,
        status: OptionalValue<Status>,
    ) -> ManagedVec<LaunchpadView<Self::Api>> {
        let (all_statuses, filter_status) = match status {
            OptionalValue::Some(status) => (false, status),
            OptionalValue::None => (true, Status::Pending),
        };
        let all_indexes = start_idx == 0 && end_idx == 0;
        let current_time = self.blockchain().get_block_timestamp();
        let mut launchpads: ManagedVec<LaunchpadView<Self::Api>> = ManagedVec::new();
        let mut real_idx = 0;
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            let mut launchpad = self.launchpads(i).get();
            launchpad.status = launchpad.get_status(current_time);
            let status_ok = all_statuses || launchpad.status == filter_status;
            let idx_ok = all_indexes || (real_idx >= start_idx && real_idx <= end_idx);
            if status_ok && idx_ok {
                launchpads.push(LaunchpadView {
                    bought: self.user_participation(&address, i).get(),
                    whitelisted: self.whitelisted_users(i).contains(&address) || !launchpad.kyc_enforced,
                    launchpad,
                });
            }
            real_idx += 1;
        }

        launchpads
    }
    
    #[view(getLaunchpadsCount)]
    fn get_launchpads_count(&self, status: OptionalValue<Status>) -> u64 {
        let (all_statuses, filter_status) = match status {
            OptionalValue::Some(status) => (false, status),
            OptionalValue::None => (true, Status::Pending),
        };
        let current_time = self.blockchain().get_block_timestamp();
        let mut count = 0;
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            if all_statuses || self.launchpads(i).get().get_status(current_time) == filter_status {
                count += 1;
            }
        }

        count
    }

    #[view(getAllLaunchpadsSince)]
    fn get_all_launchpads_since(&self, timestamp: u64) -> ManagedVec<Launchpad<Self::Api>> {
        let current_time = self.blockchain().get_block_timestamp();
        let mut launchpads: ManagedVec<Launchpad<Self::Api>> = ManagedVec::new();
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            let mut launchpad = self.launchpads(i).get();
            if launchpad.end_time > timestamp {
                launchpad.status = launchpad.get_status(current_time);
                launchpads.push(launchpad);
            }
        }

        launchpads
    }

    #[view(getActiveLaunchpads)]
    fn get_active_launchpads(&self) -> ManagedVec<Launchpad<Self::Api>> {
        let now = self.blockchain().get_block_timestamp();
        let mut launchpads: ManagedVec<Launchpad<Self::Api>> = ManagedVec::new();
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            let launchpad = self.launchpads(i).get();
            if launchpad.is_active(now) {
                launchpads.push(launchpad);
            }
        }

        launchpads
    }

    #[view(getEndedLaunchpadsNotDeployed)]
    fn get_ended_launchpads_not_deployed(&self) -> ManagedVec<Launchpad<Self::Api>> {
        let now = self.blockchain().get_block_timestamp();
        let mut launchpads: ManagedVec<Launchpad<Self::Api>> = ManagedVec::new();
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            let launchpad = self.launchpads(i).get();
            if !launchpad.deployed && !launchpad.is_active(now) {
                launchpads.push(launchpad);
            }
        }

        launchpads
    }

    #[view(getTotalRaised)]
    fn get_total_raised(&self) -> ManagedVec<EsdtTokenPayment<Self::Api>> {
        let mut raised: ManagedVec<EsdtTokenPayment<Self::Api>> = ManagedVec::new();
        for i in 0..self.last_launchpad_id().get() {
            if self.launchpads(i).is_empty() {
                continue
            }

            let launchpad = self.launchpads(i).get();
            let mut found = false;
            for i in 0..raised.len() {
                let mut payment = raised.get(i);
                if payment.token_identifier == launchpad.payment_token {
                    payment.amount += &launchpad.total_raised;
                    let _ = raised.set(i, payment);
                    found = true;
                    break;
                }
            }
            if !found {
                let payment = EsdtTokenPayment::new(launchpad.payment_token, 0, launchpad.total_raised);
                raised.push(payment);
            }
        }

        raised
    }

    #[view(getLastLaunchpadId)]
    #[storage_mapper("last_launchpad_id")]
    fn last_launchpad_id(&self) -> SingleValueMapper<u64>;

    #[view(getLaunchpadIdByToken)]
    #[storage_mapper("token_launchpad_id")]
    fn token_launchpad_id(&self, token: &TokenIdentifier) -> SingleValueMapper<u64>;

    #[view(isTokenLaunched)]
    fn is_token_launched(&self, token: TokenIdentifier) -> bool {
        for launchpad_id in 0..self.last_launchpad_id().get() {
            if self.launchpads(launchpad_id).is_empty() {
                continue
            }

            let launchpad = self.launchpads(launchpad_id).get();
            if launchpad.token == token {
                return true
            }
        }

        false
    }

    #[view(getLaunchpadUsers)]
    #[storage_mapper("launchpad_users")]
    fn launchpad_users(&self, id: u64) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getUserLaunchpads)]
    #[storage_mapper("user_launchpads")]
    fn user_launchpads(&self, user: &ManagedAddress) -> UnorderedSetMapper<u64>;

    #[view(getUserParticipation)]
    #[storage_mapper("user_participation")]
    fn user_participation(&self, user: &ManagedAddress, id: u64) -> SingleValueMapper<BigUint>;

    #[view(getDeployedLaunchpadId)]
    #[storage_mapper("deployed_launchpads")]
    fn deployed_launchpads(&self, address: ManagedAddress) -> SingleValueMapper<u64>;

    #[view(getDeployedLaunchpadByAddress)]
    fn get_ended_launchpad(&self, address: ManagedAddress) -> Launchpad<Self::Api> {
        self.launchpads(self.deployed_launchpads(address).get()).get()
    }

    // kyc
    #[view(getWhitelistedUsers)]
    #[storage_mapper("whitelisted_users")]
    fn whitelisted_users(&self, id: u64) -> UnorderedSetMapper<ManagedAddress>;

    // helpers
    fn only_dao(&self) {
        require!(self.blockchain().get_caller() == self.main_dao().get(), ERROR_ONLY_MAIN_DAO);
    }

    fn only_launchpad_owner(&self, id: u64) {
        require!(!self.launchpads(id).is_empty(), ERROR_LAUNCHPAD_NOT_FOUND);

        let launchpad = self.launchpads(id).get();
        require!(self.blockchain().get_caller() == launchpad.owner, ERROR_ONLY_LAUNCHPAD_OWNER);
    }

    // proxies
    #[proxy]
    fn dao_contract_proxy(&self) -> tfn_dao::Proxy<Self::Api>;

    #[proxy]
    fn dex_contract_proxy(&self) -> tfn_dex::Proxy<Self::Api>;
}
