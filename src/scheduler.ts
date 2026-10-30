export function concurrencyLimit(value: unknown = 2) {
 const limit = Number(value);
 if (!Number.isInteger(limit) || limit < 1 || limit > 8) throw new Error('WORKFLOW_FACTORY_CONCURRENCY must be an integer from 1 to 8');
 return limit;
}
