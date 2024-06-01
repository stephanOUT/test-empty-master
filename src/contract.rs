#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg, SubMsg, CosmosMsg, StdError, SubMsgResult, Reply};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SlaveInstantiateMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:test-empty-master";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => try_increment(deps),
        ExecuteMsg::DeploySlave {count} => deploy_slave(deps, _env, info, count),
    }
}

pub fn try_increment(deps: DepsMut) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.count += 1;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "try_increment"))
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
    }
}

fn query_count(deps: Deps) -> StdResult<CountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(CountResponse { count: state.count })
}

pub fn deploy_slave(mut deps: DepsMut, env: Env, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    let instantiate_message: WasmMsg = WasmMsg::Instantiate {
        admin: Some(env.contract.address.to_string()),
        code_id: 9552,
        msg: to_binary(&SlaveInstantiateMsg {
            count: count
        })?,
        funds: vec![],
        label: "DeployedSlave".to_string(),
    };

    let sub_msg: SubMsg = SubMsg::reply_always(CosmosMsg::Wasm(instantiate_message.into()), INSTANTIATE_REPLY_ID);

    Ok(Response::new()
        .add_attribute("method", "DeployedSlave")
        .add_submessage(sub_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        INSTANTIATE_REPLY_ID => handle_instantiate_reply(deps, msg),

        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

pub fn handle_instantiate_reply(deps: DepsMut, msg: Reply) -> StdResult<Response> {
    deps.api.debug(&format!("Status 1"));

    // Ensure the result is parsed correctly
    let result = match msg.result {
        SubMsgResult::Ok(result) => result,
        SubMsgResult::Err(err) => {
            deps.api.debug(&format!("SubMsg error: {}", err));
            return Err(StdError::generic_err(format!("SubMsg error: {}", err)));
        }
    };

    deps.api.debug(&format!("Status 2"));

    // Log all events for debugging purposes
    deps.api.debug("Handling instantiate reply");
    for event in &result.events {
        deps.api.debug(&format!("Event: {}", event.ty));
        for attr in &event.attributes {
            deps.api.debug(&format!("{}: {}", attr.key, attr.value));
        }
    }

    deps.api.debug(&format!("Status 3"));

    // Find the event type "instantiate_contract" which contains the contract_address
    let event = match result.events.iter().find(|event| event.ty == "instantiate") {
        Some(event) => event,
        None => {
            deps.api.debug("Cannot find `instantiate` event");
            return Err(StdError::generic_err("Cannot find `instantiate` event"));
        }
    };

    deps.api.debug(&format!("Status 4"));

    // Find the contract_address from the "instantiate" event
    let contract_address = match event.attributes.iter().find(|attr| attr.key == "_contract_address") {
        Some(attr) => &attr.value,
        None => {
            deps.api.debug("Cannot find `_contract_address` attribute");
            return Err(StdError::generic_err("Cannot find `_contract_address` attribute"));
        }
    };

    deps.api.debug(&format!("Status 5"));

    // Construct the response and include relevant attributes
    Ok(Response::new()
        .add_attribute("method", "handle_instantiate_reply")
        .add_attribute("contract_address", contract_address))
}


#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }
}
