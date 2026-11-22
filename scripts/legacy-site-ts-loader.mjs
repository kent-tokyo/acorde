// Run with --loader to resolve the site's extensionless TypeScript imports.
export async function resolve(specifier, context, next) {
  try { return await next(specifier, context); } catch (error) {
    if (error.code === 'ERR_MODULE_NOT_FOUND' && specifier.startsWith('.') && !/\.[a-z]+$/.test(specifier)) return next(`${specifier}.ts`, context);
    throw error;
  }
}
