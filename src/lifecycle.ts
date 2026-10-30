export interface SignalSource {
 on(event: 'SIGINT' | 'SIGTERM', listener: () => void): unknown;
 off(event: 'SIGINT' | 'SIGTERM', listener: () => void): unknown;
}
export function installShutdownSignals(source: SignalSource, quit: () => Promise<void>, report: (error: unknown) => void) {
 const handler = () => { void quit().catch(report); };
 source.on('SIGINT',handler); source.on('SIGTERM',handler);
 return () => { source.off('SIGINT',handler); source.off('SIGTERM',handler); };
}
interface Reply {
 destroyed: boolean;
 once(event: 'finish' | 'close', listener: () => void): unknown;
 off(event: 'finish' | 'close', listener: () => void): unknown;
}
export function closeAfterReply(reply: Reply, close: () => Promise<void>, report: (error: unknown) => void) {
 const finish = () => { reply.off('finish',finish); reply.off('close',finish); void close().catch(report); };
 if (reply.destroyed) finish(); else { reply.once('finish',finish); reply.once('close',finish); }
}
