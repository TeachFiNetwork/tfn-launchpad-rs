pub static ERROR_WRONG_MAIN_DAO_SHARD: &[u8] = b"main DAO is on another shard";
pub static ERROR_WRONG_TEMPLATE_DAO_SHARD: &[u8] = b"template DAO is on another shard";
pub static ERROR_ONLY_MAIN_DAO: &[u8] = b"only the main DAO can execute this action";
pub static ERROR_ONLY_LAUNCHPAD_OWNER: &[u8] = b"only the launchpad owner can execute this action";
pub static ERROR_NOT_ACTIVE: &[u8] = b"contract is paused";
pub static ERROR_WRONG_START_TIME: &[u8] = b"start time can not be in the past";
pub static ERROR_WRONG_END_TIME: &[u8] = b"end time must be after start time";
pub static ERROR_TOKEN_ALREADY_LAUNCHED: &[u8] = b"token already launched";
pub static ERROR_WRONG_MIN_MAX_AMOUNTS: &[u8] = b"max buy amount must be greater than min buy amount";
pub static ERROR_ZERO_PRICE: &[u8] = b"price can not be zero";
pub static ERROR_LAUNCHPAD_NOT_FOUND: &[u8] = b"a launchpad with this id does not exist";
pub static ERROR_LAUNCHPAD_INACTIVE: &[u8] = b"launchpad not active";
pub static ERROR_WRONG_TOKEN: &[u8] = b"wrong payment token";
pub static ERROR_LOW_AMOUNT: &[u8] = b"must buy at least min amount";
pub static ERROR_HIGH_AMOUNT: &[u8] = b"total user bought amount exceeds max amount";
pub static ERROR_INSUFFICIENT_FUNDS: &[u8] = b"insufficient funds left in contract";
pub static ERROR_NOT_WHITELISTED: &[u8] = b"user not whitelisted";
pub static ERROR_LAUNCHPAD_NOT_ENDED: &[u8] = b"launchpad end time not reached";
pub static ERROR_ALREADY_DEPLOYED: &[u8] = b"franchise already deployed";
pub static ERROR_DELETING_LAUNCHPAD: &[u8] = b"can not delete a launchpad when tokens were sold";
pub static ERROR_ONLY_OWNER_OR_DAO: &[u8] = b"only the owner or the main DAO can execute this action";
