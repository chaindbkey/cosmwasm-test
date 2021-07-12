use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, CanonicalAddr, CosmosMsg, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128,
};

use crate::msg::{HandleMsg, InitMsg, QueryMsg, ReceiverResponse};
use crate::state::{config, config_read, State};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let receiver = HumanAddr::from(msg.receiver);
    let state = State {
        receiver: deps.api.canonical_address(&receiver)?,
        owner: deps.api.canonical_address(&env.message.sender)?,
    };

    config(&mut deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::TokenSend {} => try_tokensend(deps, env),
        HandleMsg::ResetReceiver { receiver } => try_reset(
            deps,
            env,
            deps.api.canonical_address(&HumanAddr::from(receiver))?,
        ),
    }
}

pub fn try_tokensend<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let funds = env.message.sent_funds;
    if funds
        .clone()
        .into_iter()
        .find(|x| x.denom == "uusd" && x.amount > Uint128(0))
        .is_none()
    {
        return Err(StdError::generic_err("You must pass some UST"));
    }

    let state = config_read(&deps.storage).load()?;
    let recipient = deps.api.human_address(&state.receiver)?;
    let log = vec![log("action", "send"), log("recipient", recipient.as_str())];
    let from_address = env.contract.address.clone();
    let to_address = recipient.clone();

    let r = HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address,
            to_address,
            amount: funds,
        })],
        log,
        data: None,
    };
    Ok(r)
}

pub fn try_reset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    receiver: CanonicalAddr,
) -> StdResult<HandleResponse> {
    let api = &deps.api;
    config(&mut deps.storage).update(|mut state| {
        if api.canonical_address(&env.message.sender)? != state.owner {
            return Err(StdError::unauthorized());
        }
        state.receiver = receiver;
        Ok(state)
    })?;
    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetReceiver {} => to_binary(&query_receiver(deps)?),
    }
}

fn query_receiver<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ReceiverResponse> {
    let state = config_read(&deps.storage).load()?;
    Ok(ReceiverResponse {
        receiver: deps.api.human_address(&state.receiver)?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_binary, StdError};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(44, &[]);

        let msg = InitMsg {
            receiver: "terra1j40dd3k6f3wmlx8h00eg5avasjygvsh3pg3g5p".to_string(),
        };
        let env = mock_env("creator", &coins(1000, "uusd"));

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(&deps, QueryMsg::GetReceiver {}).unwrap();
        let value: ReceiverResponse = from_binary(&res).unwrap();
        assert_eq!(
            "terra1j40dd3k6f3wmlx8h00eg5avasjygvsh3pg3g5p",
            value.receiver.to_string()
        );
    }

    #[test]
    fn failed_tokensend() {
        let mut deps = mock_dependencies(44, &coins(2, "token"));

        let msg = InitMsg {
            receiver: "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5".to_string(),
        };
        let env = mock_env("creator", &coins(1000, "token"));

        let _res = init(&mut deps, env, msg).unwrap();

        let env = mock_env("anyone", &[]);
        let msg = HandleMsg::TokenSend {};
        let res = handle(&mut deps, env, msg);
        match res {
            Ok(_) => panic!("expected error"),
            Err(StdError::GenericErr { msg, .. }) => {
                assert_eq!(msg, "You must pass some UST")
            }
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }

    #[test]
    fn tokensend() {
        let mut deps = mock_dependencies(44, &coins(2, "uusd"));

        let msg = InitMsg {
            receiver: "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5".to_string(),
        };
        let env = mock_env("creator", &coins(1000, "uusd"));

        let _res = init(&mut deps, env, msg).unwrap();

        let balance = coins(100, "uusd");
        let env = mock_env("anyone", &balance);
        let msg = HandleMsg::TokenSend {};

        //deps.querier.update_balance("anyone", coins(200, "token"));
        //let query_balance = deps.querier.query_all_balances("anyone");
        //println!("Balance {:#?}", query_balance);

        let res = handle(&mut deps, env, msg).unwrap();
        let msg = res.messages.get(0).expect("no message");
        assert_eq!(
            msg,
            &CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from("cosmos2contract"),
                to_address: HumanAddr::from("terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5"),
                amount: coins(100, "uusd"),
            })
        );
        assert_eq!(
            res.log,
            vec![
                log("action", "send"),
                log("recipient", "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5"),
            ]
        );
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies(44, &coins(2, "token"));

        let msg = InitMsg {
            receiver: "terra1j40dd3k6f3wmlx8h00eg5avasjygvsh3pg3g5p".to_string(),
        };
        let env = mock_env("creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // beneficiary can release it
        let unauth_env = mock_env("anyone", &coins(2, "token"));
        let msg = HandleMsg::ResetReceiver {
            receiver: "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5".to_string(),
        };
        let res = handle(&mut deps, unauth_env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the receiver
        let auth_env = mock_env("creator", &coins(2, "token"));
        let msg = HandleMsg::ResetReceiver {
            receiver: "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5".to_string(),
        };
        let _res = handle(&mut deps, auth_env, msg).unwrap();

        // should now be 5
        let res = query(&deps, QueryMsg::GetReceiver {}).unwrap();
        let value: ReceiverResponse = from_binary(&res).unwrap();
        assert_eq!(
            "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5",
            value.receiver.to_string()
        );
    }
}
