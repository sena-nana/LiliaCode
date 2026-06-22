import remoteControlCommandContract from "./remote-control-command-contract.json" with { type: "json" };

const manifest = Object.freeze(remoteControlCommandContract);

export const REMOTE_CONTROL_COMMANDS_CONTRACT = manifest;
export const REMOTE_CONTROL_STATUS_COMMAND = manifest.commands.status;
export const REMOTE_CONTROL_SET_HOST_ENABLED_COMMAND = manifest.commands.setHostEnabled;
export const REMOTE_CONTROL_SET_PC_NAME_COMMAND = manifest.commands.setPcName;
export const REMOTE_CONTROL_START_PAIRING_COMMAND = manifest.commands.startPairing;
export const REMOTE_CONTROL_CANCEL_PAIRING_COMMAND = manifest.commands.cancelPairing;
export const REMOTE_CONTROL_REVOKE_DEVICE_COMMAND = manifest.commands.revokeDevice;
export const REMOTE_CONTROL_PAIR_DEVICE_COMMAND = manifest.commands.pairDevice;
export const REMOTE_CONTROL_DISPATCH_REQUEST_COMMAND = manifest.commands.dispatchRequest;
