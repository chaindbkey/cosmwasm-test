use cosmwasm_std::{
    to_binary, Api, Binary, CanonicalAddr, Env, Extern, HandleResponse, HumanAddr, InitResponse,
    Querier, StdError, StdResult, Storage,
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
    _env: Env,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
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
        let env = mock_env("creator", &coins(1000, "ust"));

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
    fn tokensend() {
        let mut deps = mock_dependencies(44, &coins(2, "token"));

        let msg = InitMsg {
            receiver: "terra1w548z72h5mgf6cgdkrx5h7fqk3e5wdejkv22d5".to_string(),
        };
        let env = mock_env("creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // beneficiary can release it
        let env = mock_env("anyone", &coins(2, "token"));
        let msg = HandleMsg::TokenSend {};
        let _res = handle(&mut deps, env, msg).unwrap();
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
