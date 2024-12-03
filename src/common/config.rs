multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::common::errors::*;

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, PartialEq, Eq, Copy, Clone, Debug)]
pub enum State {
    Inactive,
    Active,
}

#[multiversx_sc::module]
pub trait ConfigModule {
    // state
    #[only_owner]
    #[endpoint(setStateActive)]
    fn set_state_active(&self) {
        require!(!self.template_dao().is_empty(), ERROR_TEMPLATE_DAO_NOT_SET);

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

    // template dao sc address
    #[only_owner]
    #[endpoint(setTemplateDAO)]
    fn set_template_dao(&self, address: ManagedAddress) {
        self.template_dao().set(address);
    }

    #[view(getTemplateDAO)]
    #[storage_mapper("template_dao")]
    fn template_dao(&self) -> SingleValueMapper<ManagedAddress>;
}
