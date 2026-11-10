import ts from 'typescript';

/** Inventory registrations without importing a test module or running fixtures. */
export function nodeCases(source, path) {
  const tree = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
  if (tree.parseDiagnostics.length) throw new Error(`Cannot parse tests: ${path}`);
  const names = new Set();
  for (const statement of tree.statements) {
    if (!ts.isImportDeclaration(statement) || statement.moduleSpecifier.text !== 'node:test') continue;
    const clause = statement.importClause;
    if (clause?.name) names.add(clause.name.text);
    for (const binding of clause?.namedBindings?.elements ?? []) {
      if (['test', 'it'].includes((binding.propertyName ?? binding.name).text)) names.add(binding.name.text);
    }
  }
  const ids = [];
  function visit(node) {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && names.has(node.expression.text)) {
      if (!ts.isExpressionStatement(node.parent) || node.parent.parent !== tree) throw new Error(`Tests must be direct top-level registrations: ${path}`);
      if (!node.arguments[0] || !ts.isStringLiteral(node.arguments[0])) throw new Error(`Test names must be literal strings: ${path}`);
      ids.push(`${path} :: ${node.arguments[0].text}`);
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return ids;
}

export function rustCases(output, target) {
  return output.split('\n').filter(line => line.endsWith(': test')).map(line => `${target} :: ${line.slice(0, -6)}`);
}
