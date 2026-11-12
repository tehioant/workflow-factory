import type { SignInStatus } from './transport.ts';
import type { Invoke } from './commands.ts';
export function signInCommands(invoke:Invoke){return {
 begin:()=>invoke<SignInStatus>('begin_session_sign_in'),
 read:()=>invoke<SignInStatus>('read_session_sign_in'),
 cancel:()=>invoke<SignInStatus>('cancel_session_sign_in'),
 retire:()=>invoke<SignInStatus>('retire_session_sign_in'),
};}
