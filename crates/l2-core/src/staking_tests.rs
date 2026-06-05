use crate::crypto::sha256_bytes;
use crate::{RewardRequest, StakingConfig, StakingError, StakingState, State, L2_NATIVE_GAS_ASSET};

fn id(label: &[u8]) -> crate::Hash32 {
    sha256_bytes(label)
}

fn cfg() -> StakingConfig {
    StakingConfig {
        minimum_stake_ent: 100,
        unbonding_period_blocks: 5,
        reward_asset_id: L2_NATIVE_GAS_ASSET,
    }
}

fn funded_state(accounts: &[(crate::Hash32, u128)]) -> State {
    let mut state = State::default();
    for (account, amount) in accounts {
        assert!(state
            .account_mut(*account)
            .credit(L2_NATIVE_GAS_ASSET, *amount));
    }
    state
}

#[test]
fn stake_and_delegate_debit_accounts_deterministically() {
    let validator = id(b"validator");
    let delegator = id(b"delegator");
    let config = cfg();
    let mut accounts = funded_state(&[(validator, 1_000), (delegator, 500)]);
    let mut staking = StakingState::default();

    assert_eq!(
        staking.stake(&mut accounts, &config, validator, 50),
        Err(StakingError::BelowMinimumStake)
    );
    staking
        .stake(&mut accounts, &config, validator, 200)
        .expect("validator stake");
    staking
        .delegate(&mut accounts, &config, delegator, validator, 125)
        .expect("delegation");

    let stake = staking.validators.get(&validator).expect("validator");
    assert_eq!(stake.self_bonded, 200);
    assert_eq!(stake.delegated, 125);
    assert_eq!(
        staking.delegations[&validator].get(&delegator).copied(),
        Some(125)
    );
    assert_eq!(
        accounts
            .account(validator)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        800
    );
    assert_eq!(
        accounts
            .account(delegator)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        375
    );
}

#[test]
fn undelegate_and_unbond_require_mature_unbonding_period() {
    let validator = id(b"validator");
    let delegator = id(b"delegator");
    let config = cfg();
    let mut accounts = funded_state(&[(validator, 1_000), (delegator, 500)]);
    let mut staking = StakingState::default();

    staking
        .stake(&mut accounts, &config, validator, 200)
        .expect("validator stake");
    staking
        .delegate(&mut accounts, &config, delegator, validator, 125)
        .expect("delegation");
    staking
        .undelegate(&config, delegator, validator, 75, 10)
        .expect("undelegate");

    assert_eq!(
        staking.withdraw_unbonded(&mut accounts, &config, delegator, 14),
        Err(StakingError::NoEligibleUnbonding)
    );
    assert_eq!(
        staking
            .withdraw_unbonded(&mut accounts, &config, delegator, 15)
            .expect("withdraw delegated"),
        75
    );
    assert_eq!(
        accounts
            .account(delegator)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        450
    );

    assert_eq!(
        staking.unbond(&config, validator, 125, 20),
        Err(StakingError::BelowMinimumStake)
    );
    assert_eq!(
        staking.unbond(&config, validator, 200, 20),
        Err(StakingError::ValidatorHasDelegations)
    );
    staking
        .undelegate(&config, delegator, validator, 50, 20)
        .expect("undelegate rest");
    staking
        .unbond(&config, validator, 200, 25)
        .expect("full validator exit");
    assert_eq!(
        staking
            .withdraw_unbonded(&mut accounts, &config, validator, 30)
            .expect("withdraw self stake"),
        200
    );
}

#[test]
fn rewards_are_funded_idempotent_and_round_dust_to_validator() {
    let payer = id(b"reward-payer");
    let validator = id(b"validator");
    let alice = id(b"alice");
    let bob = id(b"bob");
    let reward_id = id(b"reward-1");
    let config = cfg();
    let mut accounts = funded_state(&[
        (payer, 1_000),
        (validator, 1_000),
        (alice, 1_000),
        (bob, 1_000),
    ]);
    let mut staking = StakingState::default();

    staking
        .stake(&mut accounts, &config, validator, 100)
        .expect("stake");
    staking
        .delegate(&mut accounts, &config, alice, validator, 50)
        .expect("alice delegate");
    staking
        .delegate(&mut accounts, &config, bob, validator, 50)
        .expect("bob delegate");

    let distribution = staking
        .distribute_reward(
            &mut accounts,
            &config,
            RewardRequest {
                reward_id,
                payer,
                validator,
                amount: 101,
                commission_bps: 1_000,
            },
        )
        .expect("reward");

    assert_eq!(distribution.validator_amount, 57);
    assert_eq!(distribution.delegator_amount, 44);
    assert_eq!(staking.rewards.get(&validator).copied(), Some(57));
    assert_eq!(staking.rewards.get(&alice).copied(), Some(22));
    assert_eq!(staking.rewards.get(&bob).copied(), Some(22));
    assert_eq!(
        accounts
            .account(payer)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        899
    );
    assert_eq!(
        staking.distribute_reward(
            &mut accounts,
            &config,
            RewardRequest {
                reward_id,
                payer,
                validator,
                amount: 101,
                commission_bps: 1_000,
            },
        ),
        Err(StakingError::DuplicateReward)
    );

    assert_eq!(
        staking
            .claim_rewards(&mut accounts, &config, validator)
            .expect("claim"),
        57
    );
    assert_eq!(
        accounts
            .account(validator)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        957
    );
}

#[test]
fn reward_overflow_fails_without_debiting_payer() {
    let payer = id(b"reward-payer");
    let validator = id(b"validator");
    let reward_id = id(b"reward-overflow");
    let config = cfg();
    let mut accounts = funded_state(&[(payer, 1_000), (validator, 1_000)]);
    let mut staking = StakingState::default();

    staking
        .stake(&mut accounts, &config, validator, 100)
        .expect("stake");
    staking.rewards.insert(validator, u128::MAX);

    assert_eq!(
        staking.distribute_reward(
            &mut accounts,
            &config,
            RewardRequest {
                reward_id,
                payer,
                validator,
                amount: 1,
                commission_bps: 0,
            },
        ),
        Err(StakingError::Overflow)
    );
    assert!(!staking.processed_rewards.contains(&reward_id));
    assert_eq!(
        accounts
            .account(payer)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        1_000
    );
}
