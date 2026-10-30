export const block = (id = 'a', x = 40, y = 50) => ({ id, type: 'ai', name: `Block ${id}`, prompt: 'Summarize this', position: { x, y } });
export const definition = () => ({ version: 1, id: 'workflow-1', name: 'Research', blocks: [block('a'), block('b', 400, 50)], connections: [{ from: 'a', to: 'b' }] });
