//! AST to HIR lowering
//!
//! Converts SWC's TypeScript AST into our HIR representation.

use anyhow::{anyhow, Result};
use perry_types::{FuncId, GlobalId, LocalId, Type, TypeParam};
use swc_ecma_ast as ast;
use std::collections::HashSet;

use crate::ir::*;

/// Context for lowering, tracks variable bindings
pub struct LoweringContext {
    /// Counter for generating unique local IDs
    next_local_id: LocalId,
    /// Counter for generating unique global IDs
    next_global_id: GlobalId,
    /// Counter for generating unique function IDs
    next_func_id: FuncId,
    /// Counter for generating unique class IDs
    next_class_id: ClassId,
    /// Counter for generating unique enum IDs
    next_enum_id: EnumId,
    /// Counter for generating unique interface IDs
    next_interface_id: InterfaceId,
    /// Counter for generating unique type alias IDs
    next_type_alias_id: TypeAliasId,
    /// Current scope's local variables: name -> (id, type)
    locals: Vec<(String, LocalId, Type)>,
    /// Global variables: name -> (id, type)
    globals: Vec<(String, GlobalId, Type)>,
    /// Functions: name -> id
    functions: Vec<(String, FuncId)>,
    /// Function parameter defaults: func_id -> (defaults, param_local_ids)
    func_defaults: Vec<(FuncId, Vec<Option<Expr>>, Vec<LocalId>)>,
    /// Classes: name -> id
    classes: Vec<(String, ClassId)>,
    /// Static members of classes: class_name -> (static_field_names, static_method_names)
    class_statics: Vec<(String, Vec<String>, Vec<String>)>,
    /// Enums: name -> (id, members with values)
    enums: Vec<(String, EnumId, Vec<(String, EnumValue)>)>,
    /// Interfaces: name -> id
    interfaces: Vec<(String, InterfaceId)>,
    /// Type aliases: name -> (id, type_params, aliased_type)
    type_aliases: Vec<(String, TypeAliasId, Vec<TypeParam>, Type)>,
    /// Imported functions: local_name -> original_name (the exported name in the source module)
    imported_functions: Vec<(String, String)>,
    /// Native module imports: local_name -> (module_name, method_name)
    /// For namespace imports (import * as x), method_name is None
    /// For named imports (import { v4 as uuid }), method_name is Some("v4")
    native_modules: Vec<(String, String, Option<String>)>,
    /// Built-in module aliases from require(): local_name -> module_name (e.g., "myFs" -> "fs")
    builtin_module_aliases: Vec<(String, String)>,
    /// Stack of type parameter scopes (for nested generics)
    type_param_scopes: Vec<HashSet<String>>,
    /// Native class instances: local_name -> (module_name, class_name)
    /// Tracks variables that hold instances of native module classes (e.g., EventEmitter)
    native_instances: Vec<(String, String, String)>,
    /// Current class being lowered (for arrow function `this` capture)
    current_class: Option<String>,
    /// Extern function types: name -> (param_types, return_type)
    /// Stores type information for declare function statements (FFI)
    extern_func_types: Vec<(String, Vec<Type>, Type)>,
    /// Source file path (for import.meta.url)
    source_file_path: String,
    /// Variables that hold closures or other values needing cross-module export globals
    /// (arrow functions, object literals, call expressions, arrays, new expressions)
    exportable_object_vars: HashSet<String>,
}

impl LoweringContext {
    pub fn new(source_file_path: impl Into<String>) -> Self {
        Self {
            next_local_id: 0,
            next_global_id: 0,
            next_func_id: 0,
            next_class_id: 1, // Start at 1 so 0 can be used as "no parent" sentinel
            next_enum_id: 0,
            next_interface_id: 0,
            next_type_alias_id: 0,
            locals: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            func_defaults: Vec::new(),
            classes: Vec::new(),
            class_statics: Vec::new(),
            enums: Vec::new(),
            interfaces: Vec::new(),
            type_aliases: Vec::new(),
            imported_functions: Vec::new(),
            native_modules: Vec::new(),
            builtin_module_aliases: Vec::new(),
            type_param_scopes: Vec::new(),
            native_instances: Vec::new(),
            current_class: None,
            extern_func_types: Vec::new(),
            source_file_path: source_file_path.into(),
            exportable_object_vars: HashSet::new(),
        }
    }

    fn fresh_interface(&mut self) -> InterfaceId {
        let id = self.next_interface_id;
        self.next_interface_id += 1;
        id
    }

    fn fresh_type_alias(&mut self) -> TypeAliasId {
        let id = self.next_type_alias_id;
        self.next_type_alias_id += 1;
        id
    }

    /// Enter a new type parameter scope (for generic function/class)
    fn enter_type_param_scope(&mut self, type_params: &[TypeParam]) {
        let scope: HashSet<String> = type_params.iter().map(|p| p.name.clone()).collect();
        self.type_param_scopes.push(scope);
    }

    /// Exit the current type parameter scope
    fn exit_type_param_scope(&mut self) {
        self.type_param_scopes.pop();
    }

    /// Check if a name is a type parameter in the current scope
    fn is_type_param(&self, name: &str) -> bool {
        self.type_param_scopes.iter().any(|scope| scope.contains(name))
    }

    fn fresh_local(&mut self) -> LocalId {
        let id = self.next_local_id;
        self.next_local_id += 1;
        id
    }

    fn fresh_global(&mut self) -> GlobalId {
        let id = self.next_global_id;
        self.next_global_id += 1;
        id
    }

    fn fresh_func(&mut self) -> FuncId {
        let id = self.next_func_id;
        self.next_func_id += 1;
        id
    }

    fn fresh_class(&mut self) -> ClassId {
        let id = self.next_class_id;
        self.next_class_id += 1;
        id
    }

    fn fresh_enum(&mut self) -> EnumId {
        let id = self.next_enum_id;
        self.next_enum_id += 1;
        id
    }

    fn lookup_class(&self, name: &str) -> Option<ClassId> {
        self.classes.iter().find(|(n, _)| n == name).map(|(_, id)| *id)
    }

    fn register_class_statics(&mut self, class_name: String, static_fields: Vec<String>, static_methods: Vec<String>) {
        self.class_statics.push((class_name, static_fields, static_methods));
    }

    fn has_static_field(&self, class_name: &str, field_name: &str) -> bool {
        self.class_statics.iter()
            .find(|(cn, _, _)| cn == class_name)
            .map(|(_, fields, _)| fields.contains(&field_name.to_string()))
            .unwrap_or(false)
    }

    fn has_static_method(&self, class_name: &str, method_name: &str) -> bool {
        self.class_statics.iter()
            .find(|(cn, _, _)| cn == class_name)
            .map(|(_, _, methods)| methods.contains(&method_name.to_string()))
            .unwrap_or(false)
    }

    fn define_enum(&mut self, name: String, id: EnumId, members: Vec<(String, EnumValue)>) {
        self.enums.push((name, id, members));
    }

    fn lookup_enum(&self, name: &str) -> Option<(EnumId, &[(String, EnumValue)])> {
        self.enums.iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, id, members)| (*id, members.as_slice()))
    }

    fn lookup_enum_member(&self, enum_name: &str, member_name: &str) -> Option<&EnumValue> {
        self.enums.iter()
            .find(|(n, _, _)| n == enum_name)
            .and_then(|(_, _, members)| {
                members.iter()
                    .find(|(m, _)| m == member_name)
                    .map(|(_, v)| v)
            })
    }

    fn define_local(&mut self, name: String, ty: Type) -> LocalId {
        let id = self.fresh_local();
        self.locals.push((name, id, ty));
        id
    }

    fn lookup_local(&self, name: &str) -> Option<LocalId> {
        self.locals.iter().rev().find(|(n, _, _)| n == name).map(|(_, id, _)| *id)
    }

    fn lookup_local_type(&self, name: &str) -> Option<&Type> {
        self.locals.iter().rev().find(|(n, _, _)| n == name).map(|(_, _, ty)| ty)
    }

    fn lookup_func(&self, name: &str) -> Option<FuncId> {
        self.functions.iter().find(|(n, _)| n == name).map(|(_, id)| *id)
    }

    fn lookup_func_defaults(&self, func_id: FuncId) -> Option<(&[Option<Expr>], &[LocalId])> {
        self.func_defaults.iter()
            .find(|(id, _, _)| *id == func_id)
            .map(|(_, defaults, param_ids)| (defaults.as_slice(), param_ids.as_slice()))
    }

    /// Substitute parameter references in a default expression.
    /// Replaces LocalGet(callee_param_id) with the corresponding caller argument expression.
    fn substitute_param_refs_in_default(expr: &Expr, param_map: &[(LocalId, Expr)]) -> Expr {
        match expr {
            Expr::LocalGet(id) => {
                // Check if this LocalGet references one of the callee's parameters
                for (param_id, replacement) in param_map {
                    if id == param_id {
                        return replacement.clone();
                    }
                }
                // Not a parameter reference - keep as-is
                expr.clone()
            }
            Expr::Array(elements) => {
                Expr::Array(elements.iter().map(|e| Self::substitute_param_refs_in_default(e, param_map)).collect())
            }
            Expr::Object(fields) => {
                Expr::Object(fields.iter().map(|(k, v)| (k.clone(), Self::substitute_param_refs_in_default(v, param_map))).collect())
            }
            Expr::Binary { op, left, right } => {
                Expr::Binary {
                    op: *op,
                    left: Box::new(Self::substitute_param_refs_in_default(left, param_map)),
                    right: Box::new(Self::substitute_param_refs_in_default(right, param_map)),
                }
            }
            Expr::Compare { op, left, right } => {
                Expr::Compare {
                    op: *op,
                    left: Box::new(Self::substitute_param_refs_in_default(left, param_map)),
                    right: Box::new(Self::substitute_param_refs_in_default(right, param_map)),
                }
            }
            Expr::Logical { op, left, right } => {
                Expr::Logical {
                    op: *op,
                    left: Box::new(Self::substitute_param_refs_in_default(left, param_map)),
                    right: Box::new(Self::substitute_param_refs_in_default(right, param_map)),
                }
            }
            Expr::Unary { op, operand } => {
                Expr::Unary {
                    op: *op,
                    operand: Box::new(Self::substitute_param_refs_in_default(operand, param_map)),
                }
            }
            Expr::Call { callee, args, type_args } => {
                Expr::Call {
                    callee: Box::new(Self::substitute_param_refs_in_default(callee, param_map)),
                    args: args.iter().map(|a| Self::substitute_param_refs_in_default(a, param_map)).collect(),
                    type_args: type_args.clone(),
                }
            }
            Expr::Conditional { condition, then_expr, else_expr } => {
                Expr::Conditional {
                    condition: Box::new(Self::substitute_param_refs_in_default(condition, param_map)),
                    then_expr: Box::new(Self::substitute_param_refs_in_default(then_expr, param_map)),
                    else_expr: Box::new(Self::substitute_param_refs_in_default(else_expr, param_map)),
                }
            }
            Expr::PropertyGet { object, property } => {
                Expr::PropertyGet {
                    object: Box::new(Self::substitute_param_refs_in_default(object, param_map)),
                    property: property.clone(),
                }
            }
            Expr::IndexGet { object, index } => {
                Expr::IndexGet {
                    object: Box::new(Self::substitute_param_refs_in_default(object, param_map)),
                    index: Box::new(Self::substitute_param_refs_in_default(index, param_map)),
                }
            }
            Expr::New { class_name, args, type_args } => {
                Expr::New {
                    class_name: class_name.clone(),
                    args: args.iter().map(|a| Self::substitute_param_refs_in_default(a, param_map)).collect(),
                    type_args: type_args.clone(),
                }
            }
            // Leaf expressions that don't contain LocalGet - return as-is
            _ => expr.clone(),
        }
    }

    fn lookup_imported_func(&self, name: &str) -> Option<&str> {
        self.imported_functions.iter().find(|(n, _)| n == name).map(|(_, orig)| orig.as_str())
    }

    fn register_imported_func(&mut self, local_name: String, original_name: String) {
        self.imported_functions.push((local_name, original_name));
    }

    fn register_extern_func_types(&mut self, name: String, param_types: Vec<Type>, return_type: Type) {
        self.extern_func_types.push((name, param_types, return_type));
    }

    fn lookup_extern_func_types(&self, name: &str) -> Option<(&Vec<Type>, &Type)> {
        self.extern_func_types
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, params, ret)| (params, ret))
    }

    fn register_native_module(&mut self, local_name: String, module_name: String, method_name: Option<String>) {
        self.native_modules.push((local_name, module_name, method_name));
    }

    fn lookup_native_module(&self, name: &str) -> Option<(&str, Option<&str>)> {
        self.native_modules.iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, m, method)| (m.as_str(), method.as_ref().map(|s| s.as_str())))
    }

    fn register_builtin_module_alias(&mut self, local_name: String, module_name: String) {
        self.builtin_module_aliases.push((local_name, module_name));
    }

    fn lookup_builtin_module_alias(&self, name: &str) -> Option<&str> {
        self.builtin_module_aliases.iter().find(|(n, _)| n == name).map(|(_, m)| m.as_str())
    }

    fn register_native_instance(&mut self, local_name: String, module_name: String, class_name: String) {
        self.native_instances.push((local_name, module_name, class_name));
    }

    fn lookup_native_instance(&self, name: &str) -> Option<(&str, &str)> {
        self.native_instances.iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, module, class)| (module.as_str(), class.as_str()))
    }

    fn enter_scope(&self) -> (usize, usize) {
        (self.locals.len(), self.native_instances.len())
    }

    fn exit_scope(&mut self, mark: (usize, usize)) {
        self.locals.truncate(mark.0);
        self.native_instances.truncate(mark.1);
    }

}

/// Extract type parameters from SWC's TsTypeParamDecl
fn extract_type_params(decl: &ast::TsTypeParamDecl) -> Vec<TypeParam> {
    decl.params
        .iter()
        .map(|p| {
            let name = p.name.sym.to_string();
            let constraint = p.constraint.as_ref().map(|c| Box::new(extract_ts_type(c)));
            let default = p.default.as_ref().map(|d| Box::new(extract_ts_type(d)));
            TypeParam {
                name,
                constraint,
                default,
            }
        })
        .collect()
}

/// Extract a Type from an SWC TypeScript type annotation
/// This version doesn't have access to type parameter context
fn extract_ts_type(ts_type: &ast::TsType) -> Type {
    extract_ts_type_with_ctx(ts_type, None)
}

/// Extract a Type from an SWC TypeScript type annotation with type parameter context
fn extract_ts_type_with_ctx(ts_type: &ast::TsType, ctx: Option<&LoweringContext>) -> Type {
    use ast::TsType::*;
    use ast::TsKeywordTypeKind::*;

    match ts_type {
        // Keyword types (primitives)
        TsKeywordType(kw) => match kw.kind {
            TsNumberKeyword => Type::Number,
            TsStringKeyword => Type::String,
            TsBooleanKeyword => Type::Boolean,
            TsBigIntKeyword => Type::BigInt,
            TsVoidKeyword => Type::Void,
            TsNullKeyword => Type::Null,
            TsUndefinedKeyword => Type::Void,
            TsAnyKeyword => Type::Any,
            TsUnknownKeyword => Type::Unknown,
            TsNeverKeyword => Type::Never,
            TsSymbolKeyword => Type::Symbol,
            TsObjectKeyword => Type::Any, // Generic object
            TsIntrinsicKeyword => Type::Any,
        },

        // Array type: T[]
        TsArrayType(arr) => {
            let elem_type = extract_ts_type_with_ctx(&arr.elem_type, ctx);
            Type::Array(Box::new(elem_type))
        }

        // Tuple type: [T, U, V]
        TsTupleType(tuple) => {
            let elem_types: Vec<Type> = tuple
                .elem_types
                .iter()
                .map(|elem| extract_ts_type_with_ctx(&elem.ty, ctx))
                .collect();
            Type::Tuple(elem_types)
        }

        // Union type: A | B | C
        TsUnionOrIntersectionType(union_or_inter) => {
            match union_or_inter {
                ast::TsUnionOrIntersectionType::TsUnionType(union) => {
                    let types: Vec<Type> = union
                        .types
                        .iter()
                        .map(|t| extract_ts_type_with_ctx(t, ctx))
                        .collect();
                    Type::Union(types)
                }
                ast::TsUnionOrIntersectionType::TsIntersectionType(_) => {
                    // Intersection types are complex - treat as Any for now
                    Type::Any
                }
            }
        }

        // Type reference: Array<T>, MyClass, T (type param), etc.
        TsTypeRef(type_ref) => {
            let name = match &type_ref.type_name {
                ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                ast::TsEntityName::TsQualifiedName(qname) => {
                    // Qualified names like Foo.Bar
                    format!("{}.{}", get_ts_entity_name(&qname.left), qname.right.sym)
                }
            };

            // First check if this is a type parameter reference (like T, K, V)
            if let Some(context) = ctx {
                if context.is_type_param(&name) {
                    return Type::TypeVar(name);
                }
            }

            // Check for built-in generic types or generic instantiations
            if let Some(type_params) = &type_ref.type_params {
                match name.as_str() {
                    "Array" if !type_params.params.is_empty() => {
                        let elem_type = extract_ts_type_with_ctx(&type_params.params[0], ctx);
                        return Type::Array(Box::new(elem_type));
                    }
                    "Promise" if !type_params.params.is_empty() => {
                        let result_type = extract_ts_type_with_ctx(&type_params.params[0], ctx);
                        return Type::Promise(Box::new(result_type));
                    }
                    _ => {
                        // Generic type instantiation (e.g., Box<number>, Map<string, number>)
                        let type_args: Vec<Type> = type_params
                            .params
                            .iter()
                            .map(|t| extract_ts_type_with_ctx(t, ctx))
                            .collect();
                        return Type::Generic {
                            base: name,
                            type_args,
                        };
                    }
                }
            }

            Type::Named(name)
        }

        // Function type: (a: T, b: U) => R
        TsFnOrConstructorType(fn_type) => {
            match fn_type {
                ast::TsFnOrConstructorType::TsFnType(fn_ty) => {
                    // Extract parameter types
                    let params: Vec<(String, Type, bool)> = fn_ty
                        .params
                        .iter()
                        .map(|p| {
                            let (name, ty) = get_fn_param_name_and_type_with_ctx(p, ctx);
                            (name, ty, false) // TODO: detect optional params
                        })
                        .collect();

                    let return_type = extract_ts_type_with_ctx(&fn_ty.type_ann.type_ann, ctx);

                    Type::Function(perry_types::FunctionType {
                        params,
                        return_type: Box::new(return_type),
                        is_async: false,
                        is_generator: false,
                    })
                }
                ast::TsFnOrConstructorType::TsConstructorType(_) => {
                    // Constructor types are complex - treat as Any for now
                    Type::Any
                }
            }
        }

        // Literal types: "foo", 42, true
        TsLitType(lit) => match &lit.lit {
            ast::TsLit::Number(_) => Type::Number,
            ast::TsLit::Str(_) => Type::String,
            ast::TsLit::Bool(_) => Type::Boolean,
            ast::TsLit::BigInt(_) => Type::BigInt,
            ast::TsLit::Tpl(_) => Type::String,
        },

        // Parenthesized type: (T)
        TsParenthesizedType(paren) => extract_ts_type_with_ctx(&paren.type_ann, ctx),

        // Optional type: T?
        TsOptionalType(opt) => extract_ts_type_with_ctx(&opt.type_ann, ctx),

        // Rest type: ...T
        TsRestType(rest) => extract_ts_type_with_ctx(&rest.type_ann, ctx),

        // Type query: typeof x
        TsTypeQuery(_) => Type::Any,

        // Conditional type: T extends U ? X : Y
        TsConditionalType(_) => Type::Any,

        // Mapped type: { [K in T]: U }
        TsMappedType(_) => Type::Any,

        // Index access: T[K]
        TsIndexedAccessType(_) => Type::Any,

        // Infer type: infer T
        TsInferType(_) => Type::Any,

        // this type
        TsThisType(_) => Type::Any,

        // Type predicate: x is T
        TsTypePredicate(_) => Type::Boolean,

        // Import type: import("module").Type
        TsImportType(_) => Type::Any,

        // Type operator: keyof T, readonly T, unique symbol
        TsTypeOperator(_) => Type::Any,

        // Type literal: { a: T, b: U }
        TsTypeLit(_) => Type::Any, // TODO: Extract to ObjectType
    }
}

/// Helper to get name from TsEntityName
fn get_ts_entity_name(entity: &ast::TsEntityName) -> String {
    match entity {
        ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        ast::TsEntityName::TsQualifiedName(qname) => {
            format!("{}.{}", get_ts_entity_name(&qname.left), qname.right.sym)
        }
    }
}

/// Helper to get parameter name and type from TsFnParam
fn get_fn_param_name_and_type(param: &ast::TsFnParam) -> (String, Type) {
    get_fn_param_name_and_type_with_ctx(param, None)
}

/// Helper to get parameter name and type from TsFnParam with context
fn get_fn_param_name_and_type_with_ctx(param: &ast::TsFnParam, ctx: Option<&LoweringContext>) -> (String, Type) {
    match param {
        ast::TsFnParam::Ident(ident) => {
            let name = ident.id.sym.to_string();
            let ty = ident.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any);
            (name, ty)
        }
        ast::TsFnParam::Array(arr) => {
            let ty = arr.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any);
            ("_array".to_string(), ty)
        }
        ast::TsFnParam::Rest(rest) => {
            let ty = rest.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any);
            ("_rest".to_string(), ty)
        }
        ast::TsFnParam::Object(obj) => {
            let ty = obj.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any);
            ("_obj".to_string(), ty)
        }
    }
}

/// Extract type from an optional type annotation
fn extract_type_annotation(type_ann: &Option<Box<ast::TsTypeAnn>>) -> Type {
    type_ann
        .as_ref()
        .map(|ann| extract_ts_type(&ann.type_ann))
        .unwrap_or(Type::Any)
}

/// Extract class name from a member expression (e.g., "ethers.JsonRpcProvider" -> "JsonRpcProvider")
/// This is used for extends clauses that reference external module classes
fn extract_member_class_name(member: &ast::MemberExpr) -> String {
    match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        ast::MemberProp::Computed(computed) => {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = computed.expr.as_ref() {
                s.value.as_str().unwrap_or("UnknownClass").to_string()
            } else {
                "UnknownClass".to_string()
            }
        }
        ast::MemberProp::PrivateName(priv_name) => priv_name.name.to_string(),
    }
}

/// Extract type from a pattern (handles BindingIdent with type annotation)
/// Used for both parameter patterns and variable declaration bindings
fn extract_pattern_type(pat: &ast::Pat) -> Type {
    extract_pattern_type_with_ctx(pat, None)
}

/// Extract type from a pattern with type parameter context
fn extract_pattern_type_with_ctx(pat: &ast::Pat, ctx: Option<&LoweringContext>) -> Type {
    match pat {
        ast::Pat::Ident(ident) => {
            ident.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Array(arr) => {
            arr.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Rest(rest) => {
            rest.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Object(obj) => {
            obj.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, ctx))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Assign(assign) => {
            // For default parameters, get type from the left side
            extract_pattern_type_with_ctx(&assign.left, ctx)
        }
        ast::Pat::Invalid(_) | ast::Pat::Expr(_) => Type::Any,
    }
}

/// Alias for parameter type extraction (same as pattern type)
fn extract_param_type(pat: &ast::Pat) -> Type {
    extract_pattern_type(pat)
}

/// Alias for parameter type extraction with context
fn extract_param_type_with_ctx(pat: &ast::Pat, ctx: Option<&LoweringContext>) -> Type {
    extract_pattern_type_with_ctx(pat, ctx)
}

/// Extract type from a variable declaration binding
fn extract_binding_type(binding: &ast::Pat) -> Type {
    extract_pattern_type(binding)
}

/// Lower decorators from SWC AST to HIR Decorators
fn lower_decorators(_ctx: &mut LoweringContext, decorators: &[ast::Decorator]) -> Vec<Decorator> {
    decorators.iter().filter_map(|dec| {
        // The decorator expression can be:
        // - Identifier: @log
        // - Call expression: @log("prefix")
        match dec.expr.as_ref() {
            ast::Expr::Ident(ident) => {
                Some(Decorator {
                    name: ident.sym.to_string(),
                    args: Vec::new(),
                })
            }
            ast::Expr::Call(call) => {
                // Get the callee name
                if let ast::Callee::Expr(callee_expr) = &call.callee {
                    if let ast::Expr::Ident(ident) = callee_expr.as_ref() {
                        // Lower the arguments - for now just handle simple literals
                        let args: Vec<Expr> = call.args.iter()
                            .filter_map(|arg| {
                                if arg.spread.is_some() {
                                    None // Skip spread arguments for now
                                } else {
                                    // For decorator args, only handle simple literals for now
                                    match arg.expr.as_ref() {
                                        ast::Expr::Lit(lit) => lower_lit(lit).ok(),
                                        _ => None,
                                    }
                                }
                            })
                            .collect();
                        return Some(Decorator {
                            name: ident.sym.to_string(),
                            args,
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }).collect()
}

/// Lower an SWC Module to HIR Module
///
/// `source_file_path` should be the absolute path to the source file for import.meta.url support.
pub fn lower_module(ast_module: &ast::Module, name: &str, source_file_path: &str) -> Result<Module> {
    let mut ctx = LoweringContext::new(source_file_path);
    let mut module = Module::new(name);

    // Pre-scan: Find all function names that have implementations (bodies)
    // This is needed to properly handle TypeScript function overloads where
    // multiple signature-only declarations precede a single implementation
    let mut functions_with_bodies: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &ast_module.body {
        let fn_decl = match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) => Some(fn_decl),
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export_decl)) => {
                if let ast::Decl::Fn(fn_decl) = &export_decl.decl {
                    Some(fn_decl)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(fn_decl) = fn_decl {
            if fn_decl.function.body.is_some() {
                functions_with_bodies.insert(fn_decl.ident.sym.to_string());
            }
        }
    }

    // First pass: collect all function declarations (both exported and non-exported)
    // Skip 'declare function' statements (functions with no body) - they are external FFI
    // BUT: also skip overload signatures if an implementation exists
    for item in &ast_module.body {
        // Extract function declaration from both regular statements and export declarations
        let fn_decl = match item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) => Some(fn_decl),
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export_decl)) => {
                if let ast::Decl::Fn(fn_decl) = &export_decl.decl {
                    Some(fn_decl)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(fn_decl) = fn_decl {
            let func_name = fn_decl.ident.sym.to_string();

            // Skip signature-only declarations (no body)
            if fn_decl.function.body.is_none() {
                // If this function has an implementation elsewhere, skip the signature
                // (it's a TypeScript overload, not an external FFI declaration)
                if functions_with_bodies.contains(&func_name) {
                    continue;
                }

                // No implementation exists - treat as external FFI declaration
                // Extract parameter types for FFI signature
                let param_types: Vec<Type> = fn_decl.function.params.iter()
                    .map(|param| extract_param_type_with_ctx(&param.pat, None))
                    .collect();

                // Extract return type
                let return_type = fn_decl.function.return_type.as_ref()
                    .map(|rt| extract_ts_type(&rt.type_ann))
                    .unwrap_or(Type::Void);

                // Register as external function so calls resolve to ExternFuncRef
                ctx.register_imported_func(func_name.clone(), func_name.clone());
                // Also store type information for code generation
                ctx.register_extern_func_types(func_name, param_types, return_type);
                continue;
            }

            // Function has a body - register it (only once per name)
            if ctx.lookup_func(&func_name).is_none() {
                let func_id = ctx.fresh_func();
                ctx.functions.push((func_name, func_id));
            }
        }
    }

    // Second pass: lower everything
    for item in &ast_module.body {
        match item {
            ast::ModuleItem::Stmt(stmt) => {
                lower_stmt(&mut ctx, &mut module, stmt)?;
            }
            ast::ModuleItem::ModuleDecl(decl) => {
                lower_module_decl(&mut ctx, &mut module, decl)?;
            }
        }
    }

    // Populate exported_native_instances by matching native_instances with exports
    for (local_name, module_name, class_name) in &ctx.native_instances {
        // Check if this native instance is exported
        for export in &module.exports {
            if let Export::Named { local, exported } = export {
                if local == local_name {
                    module.exported_native_instances.push((
                        exported.clone(),
                        module_name.clone(),
                        class_name.clone(),
                    ));
                }
            }
        }
    }

    Ok(module)
}

fn lower_module_decl(
    ctx: &mut LoweringContext,
    module: &mut Module,
    decl: &ast::ModuleDecl,
) -> Result<()> {
    match decl {
        ast::ModuleDecl::Import(import_decl) => {
            // Get the source module path
            let raw_source = import_decl.src.value.as_str().unwrap_or("").to_string();
            // Normalize "node:" prefix (e.g., "node:async_hooks" -> "async_hooks")
            let source = raw_source.strip_prefix("node:").unwrap_or(&raw_source).to_string();

            // Check if this is a native module import
            let is_native = is_native_module(&source);

            // Parse import specifiers
            let mut specifiers = Vec::new();
            for spec in &import_decl.specifiers {
                match spec {
                    ast::ImportSpecifier::Named(named) => {
                        let local = named.local.sym.to_string();
                        let imported = named.imported
                            .as_ref()
                            .map(|i| match i {
                                ast::ModuleExportName::Ident(id) => id.sym.to_string(),
                                ast::ModuleExportName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            })
                            .unwrap_or_else(|| local.clone());
                        if is_native {
                            // Register as native module function with the original method name
                            // e.g., import { v4 as uuid } from 'uuid' -> uuid maps to uuid.v4
                            ctx.register_native_module(local.clone(), source.clone(), Some(imported.clone()));
                        } else {
                            // Register as imported function (we assume all imports are functions for now)
                            ctx.register_imported_func(local.clone(), imported.clone());
                        }
                        specifiers.push(ImportSpecifier::Named { imported, local });
                    }
                    ast::ImportSpecifier::Default(default) => {
                        let local = default.local.sym.to_string();
                        if is_native {
                            // Default import of native module (e.g., import mysql from 'mysql2/promise')
                            // Default exports don't have a method name
                            ctx.register_native_module(local.clone(), source.clone(), None);
                        } else {
                            // Default import from JS module - register so calls resolve to ExternFuncRef
                            // The original name is "default" for default exports
                            ctx.register_imported_func(local.clone(), local.clone());
                        }
                        specifiers.push(ImportSpecifier::Default { local });
                    }
                    ast::ImportSpecifier::Namespace(ns) => {
                        let local = ns.local.sym.to_string();
                        if is_native {
                            // Namespace import of native module (e.g., import * as mysql from 'mysql2')
                            // Methods are called via the namespace, so no specific method name
                            ctx.register_native_module(local.clone(), source.clone(), None);
                        } else {
                            // Namespace import from JS module - register so calls resolve to ExternFuncRef
                            ctx.register_imported_func(local.clone(), local.clone());
                        }
                        specifiers.push(ImportSpecifier::Namespace { local });
                    }
                }
            }

            // Determine module kind based on the source and whether it's native
            let module_kind = if is_native {
                ModuleKind::NativeRust
            } else {
                // Default to NativeCompiled - the compiler driver will update this
                // based on file resolution
                ModuleKind::NativeCompiled
            };

            module.imports.push(Import {
                source,
                specifiers,
                is_native,
                module_kind,
                resolved_path: None, // Will be set by compiler driver during module resolution
            });
        }
        ast::ModuleDecl::ExportDecl(export) => {
            match &export.decl {
                ast::Decl::Fn(fn_decl) => {
                    let mut func = lower_fn_decl(ctx, fn_decl)?;
                    func.is_exported = true;
                    let func_name = func.name.clone();
                    let func_id = func.id;
                    // Store parameter defaults for call-site resolution
                    let defaults: Vec<Option<Expr>> = func.params.iter().map(|p| p.default.clone()).collect();
                    let param_ids: Vec<LocalId> = func.params.iter().map(|p| p.id).collect();
                    ctx.func_defaults.push((func.id, defaults, param_ids));
                    module.functions.push(func);
                    // Track in exports
                    module.exports.push(Export::Named {
                        local: func_name.clone(),
                        exported: func_name.clone(),
                    });
                    // Track exported function for cross-module value passing
                    module.exported_functions.push((func_name, func_id));
                }
                ast::Decl::Var(var_decl) => {
                    // Handle exported variables
                    for decl in &var_decl.decls {
                        let name = get_binding_name(&decl.name)?;
                        let ty = extract_binding_type(&decl.name);
                        if let Some(init) = &decl.init {
                            // Check if this is a native class instantiation and register it
                            if let ast::Expr::New(new_expr) = init.as_ref() {
                                if let ast::Expr::Ident(class_ident) = new_expr.callee.as_ref() {
                                    let class_name = class_ident.sym.as_ref();
                                    // Map class names to their modules
                                    let module_name = match class_name {
                                        "EventEmitter" => Some("events"),
                                        "AsyncLocalStorage" => Some("async_hooks"),
                                        "WebSocket" | "WebSocketServer" => Some("ws"),
                                        "Redis" => Some("ioredis"),
                                        "LRUCache" => Some("lru-cache"),
                                        "Command" => Some("commander"),
                                        "Big" => Some("big.js"),
                                        "Decimal" => Some("decimal.js"),
                                        "BigNumber" => Some("bignumber.js"),
                                        // Database clients
                                        "Pool" => Some("pg"),
                                        "Client" => Some("pg"),
                                        _ => None,
                                    };
                                    if let Some(native_module) = module_name {
                                        ctx.register_native_instance(name.clone(), native_module.to_string(), class_name.to_string());
                                    }
                                }
                            }

                            // Check if this is an awaited native class instantiation (e.g., await new Redis())
                            if let ast::Expr::Await(await_expr) = init.as_ref() {
                                if let ast::Expr::New(new_expr) = await_expr.arg.as_ref() {
                                    if let ast::Expr::Ident(class_ident) = new_expr.callee.as_ref() {
                                        let class_name = class_ident.sym.as_ref();
                                        // Map class names to their modules
                                        let module_name = match class_name {
                                            "EventEmitter" => Some("events"),
                                            "AsyncLocalStorage" => Some("async_hooks"),
                                            "WebSocket" | "WebSocketServer" => Some("ws"),
                                            "Redis" => Some("ioredis"),
                                            "LRUCache" => Some("lru-cache"),
                                            "Command" => Some("commander"),
                                            "Big" => Some("big.js"),
                                            "Decimal" => Some("decimal.js"),
                                            "BigNumber" => Some("bignumber.js"),
                                            // Database clients
                                            "Pool" => Some("pg"),
                                            "Client" => Some("pg"),
                                            _ => None,
                                        };
                                        if let Some(native_module) = module_name {
                                            ctx.register_native_instance(name.clone(), native_module.to_string(), class_name.to_string());
                                        }
                                    }
                                }
                            }

                            // Check if this is a native module factory function call (e.g., mysql.createPool())
                            if let ast::Expr::Call(call_expr) = init.as_ref() {
                                if let ast::Callee::Expr(callee) = &call_expr.callee {
                                    if let ast::Expr::Member(member) = callee.as_ref() {
                                        if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                                            let obj_name = obj_ident.sym.as_ref();
                                            // Check if it's a known native module
                                            if let Some((module_name, _)) = ctx.lookup_native_module(obj_name) {
                                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                                    let method_name = method_ident.sym.as_ref();
                                                    // Map factory functions to their class names
                                                    let class_name = match (module_name, method_name) {
                                                        ("mysql2" | "mysql2/promise", "createPool") => Some("Pool"),
                                                        ("mysql2" | "mysql2/promise", "createConnection") => Some("Connection"),
                                                        ("pg", "connect") => Some("Client"),
                                                        _ => None,
                                                    };
                                                    if let Some(class_name) = class_name {
                                                        ctx.register_native_instance(name.clone(), module_name.to_string(), class_name.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Check if this is a direct call to a default import from a native module
                                    // e.g., Fastify() where Fastify is imported from 'fastify'
                                    if let ast::Expr::Ident(func_ident) = callee.as_ref() {
                                        let func_name = func_ident.sym.as_ref();
                                        // Check if this is a default import from a native module
                                        if let Some((module_name, None)) = ctx.lookup_native_module(func_name) {
                                            // Register as native instance - the "class" is the module name for default exports
                                            ctx.register_native_instance(name.clone(), module_name.to_string(), "App".to_string());
                                        }
                                        // Check if this is a named import that returns a handle (e.g., State from perry/ui)
                                        if let Some((module_name, Some(method_name))) = ctx.lookup_native_module(func_name) {
                                            if module_name == "perry/ui" && method_name == "State" {
                                                ctx.register_native_instance(name.clone(), module_name.to_string(), "State".to_string());
                                            }
                                        }
                                    }
                                }
                            }

                            // Track exported values that need cross-module access
                            // Include: object literals, call expressions (e.g., Router()), array literals,
                            // new expressions (e.g., new Router()), and arrow functions (e.g., () => {})
                            let needs_export_global = matches!(init.as_ref(),
                                ast::Expr::Object(_) |
                                ast::Expr::Call(_) |
                                ast::Expr::Array(_) |
                                ast::Expr::New(_) |
                                ast::Expr::Arrow(_)
                            );

                            let expr = lower_expr(ctx, init)?;
                            let id = ctx.define_local(name.clone(), ty.clone());
                            module.init.push(Stmt::Let {
                                id,
                                name: name.clone(),
                                ty,
                                mutable: matches!(var_decl.kind, ast::VarDeclKind::Let | ast::VarDeclKind::Var),
                                init: Some(expr),
                            });
                            module.exports.push(Export::Named {
                                local: name.clone(),
                                exported: name.clone(),
                            });

                            // Register exported values that need cross-module globals
                            if needs_export_global {
                                module.exported_objects.push(name.clone());
                            }

                            // Handle function aliases: export const foo = existingFunc;
                            // If the init is an identifier that refers to a function, add to exported_functions
                            if let ast::Expr::Ident(ident) = init.as_ref() {
                                let ref_name = ident.sym.to_string();
                                if let Some(func_id) = ctx.lookup_func(&ref_name) {
                                    // The exported name is an alias to an existing function
                                    module.exported_functions.push((name, func_id));
                                }
                            }
                        }
                    }
                }
                ast::Decl::Class(class_decl) => {
                    let class = lower_class_decl(ctx, class_decl, true)?;
                    let class_name = class.name.clone();
                    module.classes.push(class);
                    module.exports.push(Export::Named {
                        local: class_name.clone(),
                        exported: class_name,
                    });
                }
                ast::Decl::TsEnum(enum_decl) => {
                    let en = lower_enum_decl(ctx, enum_decl, true)?;
                    let enum_name = en.name.clone();
                    module.enums.push(en);
                    module.exports.push(Export::Named {
                        local: enum_name.clone(),
                        exported: enum_name,
                    });
                }
                ast::Decl::TsInterface(iface_decl) => {
                    let iface = lower_interface_decl(ctx, iface_decl, true)?;
                    let iface_name = iface.name.clone();
                    module.interfaces.push(iface);
                    module.exports.push(Export::Named {
                        local: iface_name.clone(),
                        exported: iface_name,
                    });
                }
                ast::Decl::TsTypeAlias(alias_decl) => {
                    let alias = lower_type_alias_decl(ctx, alias_decl, true)?;
                    let alias_name = alias.name.clone();
                    module.type_aliases.push(alias);
                    module.exports.push(Export::Named {
                        local: alias_name.clone(),
                        exported: alias_name,
                    });
                }
                _ => {}
            }
        }
        ast::ModuleDecl::ExportNamed(export_named) => {
            // export { foo, bar as baz }
            // export { foo } from "source"
            if let Some(ref src) = export_named.src {
                // Re-export from another module
                let source = src.value.as_str().unwrap_or("").to_string();
                for spec in &export_named.specifiers {
                    if let ast::ExportSpecifier::Named(named) = spec {
                        let local = match &named.orig {
                            ast::ModuleExportName::Ident(id) => id.sym.to_string(),
                            ast::ModuleExportName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                        };
                        let exported = named.exported
                            .as_ref()
                            .map(|e| match e {
                                ast::ModuleExportName::Ident(id) => id.sym.to_string(),
                                ast::ModuleExportName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            })
                            .unwrap_or_else(|| local.clone());
                        module.exports.push(Export::ReExport {
                            source: source.clone(),
                            imported: local,
                            exported,
                        });
                    }
                }
            } else {
                // Local export: export { foo, bar as baz }
                for spec in &export_named.specifiers {
                    if let ast::ExportSpecifier::Named(named) = spec {
                        let local = match &named.orig {
                            ast::ModuleExportName::Ident(id) => id.sym.to_string(),
                            ast::ModuleExportName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                        };
                        let exported = named.exported
                            .as_ref()
                            .map(|e| match e {
                                ast::ModuleExportName::Ident(id) => id.sym.to_string(),
                                ast::ModuleExportName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            })
                            .unwrap_or_else(|| local.clone());
                        module.exports.push(Export::Named { local: local.clone(), exported: exported.clone() });

                        // If the local name refers to a function, add it to exported_functions
                        // so that a wrapper function is generated for cross-module calls
                        if let Some(func_id) = ctx.lookup_func(&local) {
                            module.exported_functions.push((exported.clone(), func_id));
                        }

                        // Check if the variable is a closure or other exportable object
                        // by looking through init statements
                        for stmt in &module.init {
                            if let Stmt::Let { name, init: Some(init_expr), .. } = stmt {
                                if name == &local {
                                    let is_exportable = matches!(init_expr,
                                        Expr::Closure { .. } |
                                        Expr::Object(_) |
                                        Expr::Array(_) |
                                        Expr::Call { .. } |
                                        Expr::New { .. } |
                                        Expr::JsNew { .. }
                                    );
                                    if is_exportable {
                                        module.exported_objects.push(exported.clone());
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        ast::ModuleDecl::ExportDefaultDecl(export_default) => {
            // export default function foo() {} or export default class Foo {}
            match &export_default.decl {
                ast::DefaultDecl::Fn(fn_expr) => {
                    if let Some(ref ident) = fn_expr.ident {
                        // Named function: export default function foo() {}
                        // Create a function and mark it as default export
                        let func_name = ident.sym.to_string();
                        // TODO: properly lower function expression
                        module.exports.push(Export::Named {
                            local: func_name,
                            exported: "default".to_string(),
                        });
                    }
                }
                ast::DefaultDecl::Class(class_expr) => {
                    if let Some(ref ident) = class_expr.ident {
                        let class_name = ident.sym.to_string();
                        module.exports.push(Export::Named {
                            local: class_name,
                            exported: "default".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        ast::ModuleDecl::ExportAll(export_all) => {
            // export * from "source"
            let source = export_all.src.value.as_str().unwrap_or("").to_string();
            module.exports.push(Export::ExportAll { source });
        }
        ast::ModuleDecl::ExportDefaultExpr(_) => {
            // export default expr
            // TODO: handle this case
        }
        _ => {
            // TsImportEquals, TsExportAssignment, TsNamespaceExport - TypeScript specific
        }
    }
    Ok(())
}

fn lower_stmt(
    ctx: &mut LoweringContext,
    module: &mut Module,
    stmt: &ast::Stmt,
) -> Result<()> {
    match stmt {
        ast::Stmt::Decl(decl) => {
            match decl {
                ast::Decl::Fn(fn_decl) => {
                    // Skip declare functions (no body) - they are external FFI declarations
                    if fn_decl.function.body.is_none() {
                        return Ok(());
                    }
                    let func = lower_fn_decl(ctx, fn_decl)?;
                    // Store parameter defaults for call-site resolution
                    let defaults: Vec<Option<Expr>> = func.params.iter().map(|p| p.default.clone()).collect();
                    let param_ids: Vec<LocalId> = func.params.iter().map(|p| p.id).collect();
                    ctx.func_defaults.push((func.id, defaults, param_ids));
                    module.functions.push(func);
                }
                ast::Decl::Var(var_decl) => {
                    let mutable = var_decl.kind != ast::VarDeclKind::Const;
                    for decl in &var_decl.decls {
                        let stmts = lower_var_decl_with_destructuring(ctx, decl, mutable)?;
                        module.init.extend(stmts);
                    }
                }
                ast::Decl::Class(class_decl) => {
                    let class = lower_class_decl(ctx, class_decl, false)?;
                    module.classes.push(class);
                }
                ast::Decl::TsEnum(enum_decl) => {
                    let en = lower_enum_decl(ctx, enum_decl, false)?;
                    module.enums.push(en);
                }
                ast::Decl::TsInterface(iface_decl) => {
                    let iface = lower_interface_decl(ctx, iface_decl, false)?;
                    module.interfaces.push(iface);
                }
                ast::Decl::TsTypeAlias(alias_decl) => {
                    let alias = lower_type_alias_decl(ctx, alias_decl, false)?;
                    module.type_aliases.push(alias);
                }
                _ => {}
            }
        }
        ast::Stmt::Expr(expr_stmt) => {
            // Check if this is a destructuring assignment that needs special handling
            if let ast::Expr::Assign(assign) = expr_stmt.expr.as_ref() {
                if let ast::AssignTarget::Pat(pat) = &assign.left {
                    // This is a destructuring assignment at statement level
                    // We can emit proper Let statements for temporaries
                    let stmts = lower_destructuring_assignment_stmt(ctx, pat, &assign.right)?;
                    module.init.extend(stmts);
                    return Ok(());
                }
            }
            let expr = lower_expr(ctx, &expr_stmt.expr)?;
            module.init.push(Stmt::Expr(expr));
        }
        ast::Stmt::If(if_stmt) => {
            let condition = lower_expr(ctx, &if_stmt.test)?;
            let then_branch = lower_body_stmt(ctx, &if_stmt.cons)?;
            let else_branch = if_stmt.alt.as_ref()
                .map(|s| lower_body_stmt(ctx, s))
                .transpose()?;
            module.init.push(Stmt::If {
                condition,
                then_branch,
                else_branch,
            });
        }
        ast::Stmt::While(while_stmt) => {
            let condition = lower_expr(ctx, &while_stmt.test)?;
            let body = lower_body_stmt(ctx, &while_stmt.body)?;
            module.init.push(Stmt::While { condition, body });
        }
        ast::Stmt::For(for_stmt) => {
            // Lower the init part (can be a variable declaration or expression)
            let init = if let Some(init) = &for_stmt.init {
                match init {
                    ast::VarDeclOrExpr::VarDecl(var_decl) => {
                        // For loop initializers typically have one declaration
                        if let Some(decl) = var_decl.decls.first() {
                            let name = get_binding_name(&decl.name)?;
                            let init_expr = decl.init.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
                            let id = ctx.define_local(name.clone(), Type::Any);
                            Some(Box::new(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: true,
                                init: init_expr,
                            }))
                        } else {
                            None
                        }
                    }
                    ast::VarDeclOrExpr::Expr(expr) => {
                        Some(Box::new(Stmt::Expr(lower_expr(ctx, expr)?)))
                    }
                }
            } else {
                None
            };

            let condition = for_stmt.test.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let update = for_stmt.update.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let body = lower_body_stmt(ctx, &for_stmt.body)?;

            module.init.push(Stmt::For { init, condition, update, body });
        }
        ast::Stmt::Block(block) => {
            let stmts = lower_block_stmt(ctx, block)?;
            for stmt in stmts {
                module.init.push(stmt);
            }
        }
        ast::Stmt::Try(try_stmt) => {
            // Lower try body
            let body = lower_block_stmt(ctx, &try_stmt.block)?;

            // Lower catch clause (if present)
            let catch = if let Some(ref catch_clause) = try_stmt.handler {
                let scope_mark = ctx.enter_scope();

                let param = if let Some(ref pat) = catch_clause.param {
                    let param_name = get_pat_name(pat)?;
                    let param_id = ctx.define_local(param_name.clone(), Type::Any);
                    Some((param_id, param_name))
                } else {
                    None
                };

                let catch_body = lower_block_stmt(ctx, &catch_clause.body)?;
                ctx.exit_scope(scope_mark);

                Some(CatchClause { param, body: catch_body })
            } else {
                None
            };

            // Lower finally (if present)
            let finally = if let Some(ref finally_block) = try_stmt.finalizer {
                Some(lower_block_stmt(ctx, finally_block)?)
            } else {
                None
            };

            module.init.push(Stmt::Try { body, catch, finally });
        }
        ast::Stmt::Throw(throw_stmt) => {
            let expr = lower_expr(ctx, &throw_stmt.arg)?;
            module.init.push(Stmt::Throw(expr));
        }
        ast::Stmt::Switch(switch_stmt) => {
            let discriminant = lower_expr(ctx, &switch_stmt.discriminant)?;
            let mut cases = Vec::new();

            for case in &switch_stmt.cases {
                let test = case.test.as_ref()
                    .map(|e| lower_expr(ctx, e))
                    .transpose()?;

                let mut body = Vec::new();
                for stmt in &case.cons {
                    body.extend(lower_body_stmt(ctx, stmt)?);
                }

                cases.push(SwitchCase { test, body });
            }

            module.init.push(Stmt::Switch { discriminant, cases });
        }
        ast::Stmt::ForOf(for_of_stmt) => {
            // Desugar for-of to a regular for loop:
            // for (const x of arr) { body }
            // becomes:
            // { let __arr = arr; for (let __i = 0; __i < __arr.length; __i++) { const x = __arr[__i]; body } }

            // Lower the iterable expression (the array)
            let arr_expr = lower_expr(ctx, &for_of_stmt.right)?;

            // Create internal variables for the array and index
            let arr_id = ctx.fresh_local();
            let idx_id = ctx.fresh_local();
            // Register these in the context so they can be looked up
            ctx.locals.push((format!("__arr_{}", arr_id), arr_id, Type::Array(Box::new(Type::Any))));
            ctx.locals.push((format!("__idx_{}", idx_id), idx_id, Type::Number));

            // Store array reference: let __arr = arr
            module.init.push(Stmt::Let {
                id: arr_id,
                name: format!("__arr_{}", arr_id),
                ty: Type::Array(Box::new(Type::Any)),
                mutable: false,
                init: Some(arr_expr),
            });

            // IMPORTANT: Define iteration variables BEFORE lowering the body
            // so the body can reference them
            let item_id = ctx.fresh_local();
            ctx.locals.push((format!("__item_{}", item_id), item_id, Type::Any));

            // Pre-define all variables from the pattern so body can reference them
            let var_ids: Vec<(String, u32)> = match &for_of_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        match &decl.name {
                            ast::Pat::Ident(ident) => {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Any);
                                vec![(name, id)]
                            }
                            ast::Pat::Array(arr_pat) => {
                                let mut ids = Vec::new();
                                for elem in &arr_pat.elems {
                                    if let Some(elem_pat) = elem {
                                        if let ast::Pat::Ident(ident) = elem_pat {
                                            let name = ident.id.sym.to_string();
                                            let id = ctx.define_local(name.clone(), Type::Any);
                                            ids.push((name, id));
                                        }
                                    }
                                }
                                ids
                            }
                            ast::Pat::Object(obj_pat) => {
                                let mut ids = Vec::new();
                                for prop in &obj_pat.props {
                                    match prop {
                                        ast::ObjectPatProp::Assign(assign) => {
                                            let name = assign.key.sym.to_string();
                                            let id = ctx.define_local(name.clone(), Type::Any);
                                            ids.push((name, id));
                                        }
                                        ast::ObjectPatProp::KeyValue(kv) => {
                                            if let ast::Pat::Ident(ident) = &*kv.value {
                                                let name = ident.id.sym.to_string();
                                                let id = ctx.define_local(name.clone(), Type::Any);
                                                ids.push((name, id));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ids
                            }
                            _ => {
                                let name = get_binding_name(&decl.name)?;
                                let id = ctx.define_local(name.clone(), Type::Any);
                                vec![(name, id)]
                            }
                        }
                    } else {
                        return Err(anyhow!("for-of requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(pat) => {
                    let name = get_pat_name(pat)?;
                    let id = ctx.define_local(name.clone(), Type::Any);
                    vec![(name, id)]
                }
                _ => return Err(anyhow!("Unsupported for-of left-hand side")),
            };

            // NOW lower the body - variables are defined so body can reference them
            let mut loop_body = lower_body_stmt(ctx, &for_of_stmt.body)?;

            // Build binding statements using the pre-defined variable IDs
            let binding_stmts = match &for_of_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        let item_expr = Expr::IndexGet {
                            object: Box::new(Expr::LocalGet(arr_id)),
                            index: Box::new(Expr::LocalGet(idx_id)),
                        };

                        match &decl.name {
                            ast::Pat::Ident(_) => {
                                // Simple binding: for (const x of arr)
                                let (name, id) = var_ids[0].clone();
                                vec![Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }]
                            }
                            ast::Pat::Array(arr_pat) => {
                                // Array destructuring: for (const [a, b] of arr)
                                let mut stmts = vec![Stmt::Let {
                                    id: item_id,
                                    name: format!("__item_{}", item_id),
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }];

                                // Extract each element using pre-defined IDs
                                let mut var_idx = 0;
                                for (idx, elem) in arr_pat.elems.iter().enumerate() {
                                    if let Some(elem_pat) = elem {
                                        if let ast::Pat::Ident(_) = elem_pat {
                                            let (name, id) = var_ids[var_idx].clone();
                                            var_idx += 1;
                                            stmts.push(Stmt::Let {
                                                id,
                                                name,
                                                ty: Type::Any,
                                                mutable: false,
                                                init: Some(Expr::IndexGet {
                                                    object: Box::new(Expr::LocalGet(item_id)),
                                                    index: Box::new(Expr::Number(idx as f64)),
                                                }),
                                            });
                                        }
                                    }
                                }
                                stmts
                            }
                            ast::Pat::Object(obj_pat) => {
                                // Object destructuring: for (const { a, b } of arr)
                                let mut stmts = vec![Stmt::Let {
                                    id: item_id,
                                    name: format!("__item_{}", item_id),
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }];

                                // Extract each property using pre-defined IDs
                                let mut var_idx = 0;
                                for prop in &obj_pat.props {
                                    match prop {
                                        ast::ObjectPatProp::Assign(assign) => {
                                            let prop_name = assign.key.sym.to_string();
                                            let (name, id) = var_ids[var_idx].clone();
                                            var_idx += 1;
                                            stmts.push(Stmt::Let {
                                                id,
                                                name,
                                                ty: Type::Any,
                                                mutable: false,
                                                init: Some(Expr::PropertyGet {
                                                    object: Box::new(Expr::LocalGet(item_id)),
                                                    property: prop_name,
                                                }),
                                            });
                                        }
                                        ast::ObjectPatProp::KeyValue(kv) => {
                                            let key = match &kv.key {
                                                ast::PropName::Ident(ident) => ident.sym.to_string(),
                                                _ => continue,
                                            };
                                            if let ast::Pat::Ident(_) = &*kv.value {
                                                let (name, id) = var_ids[var_idx].clone();
                                                var_idx += 1;
                                                stmts.push(Stmt::Let {
                                                    id,
                                                    name,
                                                    ty: Type::Any,
                                                    mutable: false,
                                                    init: Some(Expr::PropertyGet {
                                                        object: Box::new(Expr::LocalGet(item_id)),
                                                        property: key,
                                                    }),
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                stmts
                            }
                            _ => {
                                let (name, id) = var_ids[0].clone();
                                vec![Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(Expr::IndexGet {
                                        object: Box::new(Expr::LocalGet(arr_id)),
                                        index: Box::new(Expr::LocalGet(idx_id)),
                                    }),
                                }]
                            }
                        }
                    } else {
                        return Err(anyhow!("for-of requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(_) => {
                    let (name, id) = var_ids[0].clone();
                    vec![Stmt::Let {
                        id,
                        name,
                        ty: Type::Any,
                        mutable: false,
                        init: Some(Expr::IndexGet {
                            object: Box::new(Expr::LocalGet(arr_id)),
                            index: Box::new(Expr::LocalGet(idx_id)),
                        }),
                    }]
                }
                _ => return Err(anyhow!("Unsupported for-of left-hand side")),
            };

            // Prepend the binding statements to the loop body
            for (i, stmt) in binding_stmts.into_iter().enumerate() {
                loop_body.insert(i, stmt);
            }

            // Create the for loop:
            // for (let __i = 0; __i < __arr.length; __i++) { ... }
            module.init.push(Stmt::For {
                init: Some(Box::new(Stmt::Let {
                    id: idx_id,
                    name: format!("__idx_{}", idx_id),
                    ty: Type::Number,
                    mutable: true,
                    init: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Compare {
                    op: CompareOp::Lt,
                    left: Box::new(Expr::LocalGet(idx_id)),
                    right: Box::new(Expr::PropertyGet {
                        object: Box::new(Expr::LocalGet(arr_id)),
                        property: "length".to_string(),
                    }),
                }),
                update: Some(Expr::Update {
                    id: idx_id,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
                body: loop_body,
            });
        }
        ast::Stmt::ForIn(for_in_stmt) => {
            // Desugar for-in to a for-of over Object.keys(obj):
            // for (const key in obj) { body }
            // becomes:
            // { let __keys = Object.keys(obj); for (let __i = 0; __i < __keys.length; __i++) { const key = __keys[__i]; body } }

            // Get the iteration variable name
            let key_name = match &for_in_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        get_binding_name(&decl.name)?
                    } else {
                        return Err(anyhow!("for-in requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(pat) => get_pat_name(pat)?,
                _ => return Err(anyhow!("Unsupported for-in left-hand side")),
            };

            // Lower the object expression
            let obj_expr = lower_expr(ctx, &for_in_stmt.right)?;

            // Create Object.keys(obj) expression to get the array of keys
            let keys_expr = Expr::ObjectKeys(Box::new(obj_expr));

            // Create internal variables for the keys array and index
            let keys_id = ctx.fresh_local();
            let idx_id = ctx.fresh_local();
            let key_id = ctx.define_local(key_name.clone(), Type::String);

            // Store keys array reference: let __keys = Object.keys(obj)
            module.init.push(Stmt::Let {
                id: keys_id,
                name: format!("__keys_{}", keys_id),
                ty: Type::Array(Box::new(Type::String)),
                mutable: false,
                init: Some(keys_expr),
            });

            // Lower the body
            let mut loop_body = lower_body_stmt(ctx, &for_in_stmt.body)?;

            // Prepend: const key = __keys[__i]
            loop_body.insert(0, Stmt::Let {
                id: key_id,
                name: key_name,
                ty: Type::String,
                mutable: false,
                init: Some(Expr::IndexGet {
                    object: Box::new(Expr::LocalGet(keys_id)),
                    index: Box::new(Expr::LocalGet(idx_id)),
                }),
            });

            // Create the for loop:
            // for (let __i = 0; __i < __keys.length; __i++) { ... }
            module.init.push(Stmt::For {
                init: Some(Box::new(Stmt::Let {
                    id: idx_id,
                    name: format!("__idx_{}", idx_id),
                    ty: Type::Number,
                    mutable: true,
                    init: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Compare {
                    op: CompareOp::Lt,
                    left: Box::new(Expr::LocalGet(idx_id)),
                    right: Box::new(Expr::PropertyGet {
                        object: Box::new(Expr::LocalGet(keys_id)),
                        property: "length".to_string(),
                    }),
                }),
                update: Some(Expr::Update {
                    id: idx_id,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
                body: loop_body,
            });
        }
        _ => {}
    }
    Ok(())
}

fn lower_fn_decl(ctx: &mut LoweringContext, fn_decl: &ast::FnDecl) -> Result<Function> {
    let name = fn_decl.ident.sym.to_string();
    let func_id = ctx.lookup_func(&name).unwrap_or_else(|| ctx.fresh_func());

    // Extract type parameters from generic function declaration (e.g., function foo<T, U>(...))
    let type_params = fn_decl.function.type_params
        .as_ref()
        .map(|tp| extract_type_params(tp))
        .unwrap_or_default();

    // Enter type parameter scope for resolving T, U, etc. in body types
    ctx.enter_type_param_scope(&type_params);

    let scope_mark = ctx.enter_scope();

    // Lower parameters with type extraction (using context for type param resolution)
    let mut params = Vec::new();
    for param in fn_decl.function.params.iter() {
        let param_name = get_pat_name(&param.pat)?;
        let param_type = extract_param_type_with_ctx(&param.pat, Some(ctx));
        let param_default = get_param_default(ctx, &param.pat)?;
        let param_id = ctx.define_local(param_name.clone(), param_type.clone());
        let is_rest = is_rest_param(&param.pat);
        params.push(Param {
            id: param_id,
            name: param_name,
            ty: param_type,
            default: param_default,
            is_rest,
        });
    }

    // Extract return type from function's type annotation (with context)
    let return_type = fn_decl.function.return_type.as_ref()
        .map(|rt| extract_ts_type_with_ctx(&rt.type_ann, Some(ctx)))
        .unwrap_or(Type::Any);

    // Lower body
    let body = if let Some(ref block) = fn_decl.function.body {
        lower_block_stmt(ctx, block)?
    } else {
        Vec::new()
    };

    ctx.exit_scope(scope_mark);

    // Exit type parameter scope
    ctx.exit_type_param_scope();

    Ok(Function {
        id: func_id,
        name,
        type_params,
        params,
        return_type,
        body,
        is_async: fn_decl.function.is_async,
        is_exported: false,
        captures: Vec::new(),
        decorators: Vec::new(),
    })
}

fn lower_class_decl(ctx: &mut LoweringContext, class_decl: &ast::ClassDecl, is_exported: bool) -> Result<Class> {
    let name = class_decl.ident.sym.to_string();
    let class_id = ctx.lookup_class(&name).unwrap_or_else(|| {
        let id = ctx.fresh_class();
        ctx.classes.push((name.clone(), id));
        id
    });

    // Set current class for arrow function `this` capture tracking
    let old_class = ctx.current_class.take();
    ctx.current_class = Some(name.clone());

    // Extract type parameters from generic class declaration (e.g., class Box<T>)
    let type_params = class_decl.class.type_params
        .as_ref()
        .map(|tp| extract_type_params(tp))
        .unwrap_or_default();

    // Enter type parameter scope for resolving T, U, etc. in member types
    ctx.enter_type_param_scope(&type_params);

    // Handle extends clause
    let (extends, extends_name, native_extends) = if let Some(ref super_class) = class_decl.class.super_class {
        if let ast::Expr::Ident(ident) = super_class.as_ref() {
            let parent_name = ident.sym.to_string();
            // First check if it's a native module class
            let native_parent = match parent_name.as_str() {
                "EventEmitter" => Some(("events".to_string(), "EventEmitter".to_string())),
                "AsyncLocalStorage" => Some(("async_hooks".to_string(), "AsyncLocalStorage".to_string())),
                "WebSocketServer" => Some(("ws".to_string(), "WebSocketServer".to_string())),
                _ => None,
            };
            if native_parent.is_some() {
                (None, None, native_parent)
            } else {
                // Always capture the parent name for imported classes that may not have a ClassId
                (ctx.lookup_class(&parent_name), Some(parent_name), None)
            }
        } else if let ast::Expr::Member(member) = super_class.as_ref() {
            // Handle member expression like ethers.JsonRpcProvider or module.ClassName
            let parent_name = extract_member_class_name(member);
            // For member expressions, we don't have ClassId - just store the name
            (None, Some(parent_name), None)
        } else {
            (None, None, None)
        }
    } else {
        (None, None, None)
    };

    // First pass: collect static field/method names for early registration
    // This allows static method bodies to reference static fields
    let mut static_field_names = Vec::new();
    let mut static_method_names = Vec::new();
    for member in &class_decl.class.body {
        match member {
            ast::ClassMember::Method(method) if method.is_static => {
                if let ast::PropName::Ident(ident) = &method.key {
                    static_method_names.push(ident.sym.to_string());
                }
            }
            ast::ClassMember::ClassProp(prop) if prop.is_static => {
                if let ast::PropName::Ident(ident) = &prop.key {
                    static_field_names.push(ident.sym.to_string());
                }
            }
            _ => {}
        }
    }

    // Register static members early so method bodies can reference them
    ctx.register_class_statics(name.clone(), static_field_names, static_method_names);

    let mut fields = Vec::new();
    let mut static_fields = Vec::new();
    let mut constructor = None;
    let mut methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut getters = Vec::new();
    let mut setters = Vec::new();

    // Second pass: actually lower the class members
    for member in &class_decl.class.body {
        match member {
            ast::ClassMember::Constructor(ctor) => {
                constructor = Some(lower_constructor(ctx, &name, ctor)?);
            }
            ast::ClassMember::Method(method) => {
                // Get the property name for getters/setters
                let prop_name = match &method.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                    _ => continue,
                };

                match method.kind {
                    ast::MethodKind::Getter => {
                        // Getter: no parameters, returns a value
                        let func = lower_getter_method(ctx, method)?;
                        getters.push((prop_name, func));
                    }
                    ast::MethodKind::Setter => {
                        // Setter: takes one parameter
                        let func = lower_setter_method(ctx, method)?;
                        setters.push((prop_name, func));
                    }
                    ast::MethodKind::Method => {
                        let func = lower_class_method(ctx, method)?;
                        if method.is_static {
                            static_methods.push(func);
                        } else {
                            methods.push(func);
                        }
                    }
                }
            }
            ast::ClassMember::ClassProp(prop) => {
                let field = lower_class_prop(ctx, prop)?;
                if prop.is_static {
                    static_fields.push(field);
                } else {
                    fields.push(field);
                }
            }
            ast::ClassMember::PrivateProp(prop) => {
                let field = lower_private_prop(ctx, prop)?;
                if prop.is_static {
                    static_fields.push(field);
                } else {
                    fields.push(field);
                }
            }
            _ => {}
        }
    }

    // Exit type parameter scope
    ctx.exit_type_param_scope();

    // Restore previous current_class
    ctx.current_class = old_class;

    Ok(Class {
        id: class_id,
        name,
        type_params,
        extends,
        extends_name,
        native_extends,
        fields,
        constructor,
        methods,
        getters,
        setters,
        static_fields,
        static_methods,
        is_exported,
    })
}

fn lower_enum_decl(ctx: &mut LoweringContext, enum_decl: &ast::TsEnumDecl, is_exported: bool) -> Result<Enum> {
    let name = enum_decl.id.sym.to_string();
    let enum_id = ctx.fresh_enum();

    let mut members = Vec::new();
    let mut next_value: i64 = 0;

    for member in &enum_decl.members {
        // Get member name
        let member_name = match &member.id {
            ast::TsEnumMemberId::Ident(ident) => ident.sym.to_string(),
            ast::TsEnumMemberId::Str(s) => s.value.as_str().unwrap_or("").to_string(),
        };

        // Get member value
        let value = if let Some(ref init) = member.init {
            match init.as_ref() {
                ast::Expr::Lit(ast::Lit::Num(n)) => {
                    let v = n.value as i64;
                    next_value = v + 1;
                    EnumValue::Number(v)
                }
                ast::Expr::Lit(ast::Lit::Str(s)) => {
                    EnumValue::String(s.value.as_str().unwrap_or("").to_string())
                }
                ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Minus => {
                    // Handle negative numbers like -1
                    if let ast::Expr::Lit(ast::Lit::Num(n)) = unary.arg.as_ref() {
                        let v = -(n.value as i64);
                        next_value = v + 1;
                        EnumValue::Number(v)
                    } else {
                        // Default to auto-increment
                        let v = next_value;
                        next_value += 1;
                        EnumValue::Number(v)
                    }
                }
                _ => {
                    // For complex expressions, default to auto-increment
                    let v = next_value;
                    next_value += 1;
                    EnumValue::Number(v)
                }
            }
        } else {
            // Auto-increment
            let v = next_value;
            next_value += 1;
            EnumValue::Number(v)
        };

        members.push(EnumMember {
            name: member_name,
            value,
        });
    }

    // Register the enum in the context for later lookups
    let member_values: Vec<(String, EnumValue)> = members.iter()
        .map(|m| (m.name.clone(), m.value.clone()))
        .collect();
    ctx.define_enum(name.clone(), enum_id, member_values);

    Ok(Enum {
        id: enum_id,
        name,
        members,
        is_exported,
    })
}

fn lower_interface_decl(ctx: &mut LoweringContext, iface_decl: &ast::TsInterfaceDecl, is_exported: bool) -> Result<Interface> {
    let name = iface_decl.id.sym.to_string();
    let iface_id = ctx.fresh_interface();

    // Extract type parameters
    let type_params = iface_decl.type_params.as_ref()
        .map(|tp| extract_type_params(tp))
        .unwrap_or_default();

    // Enter type param scope for resolving type references in body
    ctx.enter_type_param_scope(&type_params);

    // Extract extended interfaces
    let extends: Vec<Type> = iface_decl.extends.iter()
        .map(|ext| {
            let base_name = match &*ext.expr {
                ast::Expr::Ident(id) => id.sym.to_string(),
                _ => "unknown".to_string(),
            };
            // Handle type arguments if present
            if let Some(ref type_args) = ext.type_args {
                let args: Vec<Type> = type_args.params.iter()
                    .map(|t| extract_ts_type_with_ctx(t, Some(ctx)))
                    .collect();
                if args.is_empty() {
                    Type::Named(base_name)
                } else {
                    Type::Generic {
                        base: base_name,
                        type_args: args,
                    }
                }
            } else {
                Type::Named(base_name)
            }
        })
        .collect();

    // Extract properties and methods from interface body
    let mut properties = Vec::new();
    let mut methods = Vec::new();

    for member in &iface_decl.body.body {
        match member {
            ast::TsTypeElement::TsPropertySignature(prop) => {
                let prop_name = match &*prop.key {
                    ast::Expr::Ident(id) => id.sym.to_string(),
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.as_str().unwrap_or("").to_string(),
                    _ => continue,
                };
                let prop_type = prop.type_ann.as_ref()
                    .map(|ta| extract_ts_type_with_ctx(&ta.type_ann, Some(ctx)))
                    .unwrap_or(Type::Any);
                properties.push(InterfaceProperty {
                    name: prop_name,
                    ty: prop_type,
                    optional: prop.optional,
                    readonly: prop.readonly,
                });
            }
            ast::TsTypeElement::TsMethodSignature(method) => {
                let method_name = match &*method.key {
                    ast::Expr::Ident(id) => id.sym.to_string(),
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.as_str().unwrap_or("").to_string(),
                    _ => continue,
                };

                // Method's own type parameters
                let method_type_params = method.type_params.as_ref()
                    .map(|tp| extract_type_params(tp))
                    .unwrap_or_default();

                // Enter method's type param scope
                ctx.enter_type_param_scope(&method_type_params);

                // Extract parameters
                let params: Vec<(String, Type, bool)> = method.params.iter()
                    .map(|p| {
                        let (name, ty) = get_fn_param_name_and_type_with_ctx(p, Some(ctx));
                        let optional = matches!(p, ast::TsFnParam::Ident(id) if id.optional);
                        (name, ty, optional)
                    })
                    .collect();

                // Extract return type
                let return_type = method.type_ann.as_ref()
                    .map(|ta| extract_ts_type_with_ctx(&ta.type_ann, Some(ctx)))
                    .unwrap_or(Type::Void);

                ctx.exit_type_param_scope();

                methods.push(InterfaceMethod {
                    name: method_name,
                    type_params: method_type_params,
                    params,
                    return_type,
                });
            }
            _ => {} // Skip other member types for now
        }
    }

    ctx.exit_type_param_scope();

    // Register interface in context
    ctx.interfaces.push((name.clone(), iface_id));

    Ok(Interface {
        id: iface_id,
        name,
        type_params,
        extends,
        properties,
        methods,
        is_exported,
    })
}

fn lower_type_alias_decl(ctx: &mut LoweringContext, alias_decl: &ast::TsTypeAliasDecl, is_exported: bool) -> Result<TypeAlias> {
    let name = alias_decl.id.sym.to_string();
    let alias_id = ctx.fresh_type_alias();

    // Extract type parameters
    let type_params = alias_decl.type_params.as_ref()
        .map(|tp| extract_type_params(tp))
        .unwrap_or_default();

    // Enter type param scope for resolving type references
    ctx.enter_type_param_scope(&type_params);

    // Extract the aliased type
    let ty = extract_ts_type_with_ctx(&alias_decl.type_ann, Some(ctx));

    ctx.exit_type_param_scope();

    // Register type alias in context
    ctx.type_aliases.push((name.clone(), alias_id, type_params.clone(), ty.clone()));

    Ok(TypeAlias {
        id: alias_id,
        name,
        type_params,
        ty,
        is_exported,
    })
}

fn lower_constructor(ctx: &mut LoweringContext, class_name: &str, ctor: &ast::Constructor) -> Result<Function> {
    let scope_mark = ctx.enter_scope();

    // Add 'this' as a special local
    let _this_id = ctx.define_local("this".to_string(), Type::Any);

    // Lower parameters with type extraction (using context for class type param resolution)
    let mut params = Vec::new();
    for param in &ctor.params {
        match param {
            ast::ParamOrTsParamProp::Param(p) => {
                let param_name = get_pat_name(&p.pat)?;
                let param_type = extract_param_type_with_ctx(&p.pat, Some(ctx));
                let param_default = get_param_default(ctx, &p.pat)?;
                let is_rest = is_rest_param(&p.pat);
                let param_id = ctx.define_local(param_name.clone(), param_type.clone());
                params.push(Param {
                    id: param_id,
                    name: param_name,
                    ty: param_type,
                    default: param_default,
                    is_rest,
                });
            }
            ast::ParamOrTsParamProp::TsParamProp(ts_prop) => {
                // Handle parameter properties (e.g., constructor(public x: number))
                let (param_name, param_type) = match &ts_prop.param {
                    ast::TsParamPropParam::Ident(ident) => {
                        let name = ident.id.sym.to_string();
                        let ty = ident.type_ann.as_ref()
                            .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                            .unwrap_or(Type::Any);
                        (name, ty)
                    }
                    ast::TsParamPropParam::Assign(assign) => {
                        let name = get_pat_name(&assign.left)?;
                        let ty = extract_param_type_with_ctx(&assign.left, Some(ctx));
                        (name, ty)
                    }
                };
                let param_id = ctx.define_local(param_name.clone(), param_type.clone());
                params.push(Param {
                    id: param_id,
                    name: param_name,
                    ty: param_type,
                    default: None,
                    is_rest: false, // TsParamProp cannot be a rest parameter
                });
            }
        }
    }

    // Lower body
    let body = if let Some(ref block) = ctor.body {
        lower_block_stmt(ctx, block)?
    } else {
        Vec::new()
    };

    ctx.exit_scope(scope_mark);

    Ok(Function {
        id: ctx.fresh_func(),
        name: format!("{}::constructor", class_name),
        type_params: Vec::new(),
        params,
        return_type: Type::Void,
        body,
        is_async: false,
        is_exported: false,
        captures: Vec::new(),
        decorators: Vec::new(),
    })
}

fn lower_class_method(ctx: &mut LoweringContext, method: &ast::ClassMethod) -> Result<Function> {
    let name = match &method.key {
        ast::PropName::Ident(ident) => ident.sym.to_string(),
        ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
        _ => return Err(anyhow!("Unsupported method key")),
    };

    // Lower decorators from the method's function
    let decorators = lower_decorators(ctx, &method.function.decorators);

    // Extract method-level type parameters (e.g., method<U>(x: U): T)
    // Note: Class-level type params are already in scope from lower_class_decl
    let type_params = method.function.type_params
        .as_ref()
        .map(|tp| extract_type_params(tp))
        .unwrap_or_default();

    // Enter method's type param scope (nested inside class scope if applicable)
    ctx.enter_type_param_scope(&type_params);

    let scope_mark = ctx.enter_scope();

    // Add 'this' for instance methods
    if !method.is_static {
        ctx.define_local("this".to_string(), Type::Any);
    }

    // Lower parameters with type extraction (using context for type param resolution)
    let mut params = Vec::new();
    for param in &method.function.params {
        let param_name = get_pat_name(&param.pat)?;
        let param_type = extract_param_type_with_ctx(&param.pat, Some(ctx));
        let param_default = get_param_default(ctx, &param.pat)?;
        let is_rest = is_rest_param(&param.pat);
        let param_id = ctx.define_local(param_name.clone(), param_type.clone());
        params.push(Param {
            id: param_id,
            name: param_name,
            ty: param_type,
            default: param_default,
            is_rest,
        });
    }

    // Extract return type (with context)
    let return_type = method.function.return_type.as_ref()
        .map(|rt| extract_ts_type_with_ctx(&rt.type_ann, Some(ctx)))
        .unwrap_or(Type::Any);

    // Lower body
    let body = if let Some(ref block) = method.function.body {
        lower_block_stmt(ctx, block)?
    } else {
        Vec::new()
    };

    ctx.exit_scope(scope_mark);

    // Exit method's type param scope
    ctx.exit_type_param_scope();

    Ok(Function {
        id: ctx.fresh_func(),
        name,
        type_params,
        params,
        return_type,
        body,
        is_async: method.function.is_async,
        is_exported: false,
        captures: Vec::new(),
        decorators,
    })
}

/// Lower a getter method (get propertyName(): Type { ... })
fn lower_getter_method(ctx: &mut LoweringContext, method: &ast::ClassMethod) -> Result<Function> {
    let name = match &method.key {
        ast::PropName::Ident(ident) => format!("get_{}", ident.sym),
        ast::PropName::Str(s) => format!("get_{}", s.value.as_str().unwrap_or("")),
        _ => return Err(anyhow!("Unsupported getter key")),
    };

    let scope_mark = ctx.enter_scope();

    // Add 'this' for instance getters
    ctx.define_local("this".to_string(), Type::Any);

    // Getters have no parameters

    // Extract return type
    let return_type = method.function.return_type.as_ref()
        .map(|rt| extract_ts_type_with_ctx(&rt.type_ann, Some(ctx)))
        .unwrap_or(Type::Any);

    // Lower body
    let body = if let Some(ref block) = method.function.body {
        lower_block_stmt(ctx, block)?
    } else {
        Vec::new()
    };

    ctx.exit_scope(scope_mark);

    Ok(Function {
        id: ctx.fresh_func(),
        name,
        type_params: Vec::new(),
        params: Vec::new(),
        return_type,
        body,
        is_async: false,
        is_exported: false,
        captures: Vec::new(),
        decorators: Vec::new(),
    })
}

/// Lower a setter method (set propertyName(value: Type) { ... })
fn lower_setter_method(ctx: &mut LoweringContext, method: &ast::ClassMethod) -> Result<Function> {
    let name = match &method.key {
        ast::PropName::Ident(ident) => format!("set_{}", ident.sym),
        ast::PropName::Str(s) => format!("set_{}", s.value.as_str().unwrap_or("")),
        _ => return Err(anyhow!("Unsupported setter key")),
    };

    let scope_mark = ctx.enter_scope();

    // Add 'this' for instance setters
    ctx.define_local("this".to_string(), Type::Any);

    // Setters have exactly one parameter
    let mut params = Vec::new();
    for param in &method.function.params {
        let param_name = get_pat_name(&param.pat)?;
        let param_type = extract_param_type_with_ctx(&param.pat, Some(ctx));
        let param_id = ctx.define_local(param_name.clone(), param_type.clone());
        params.push(Param {
            id: param_id,
            name: param_name,
            ty: param_type,
            default: None,
            is_rest: false,
        });
    }

    // Lower body
    let body = if let Some(ref block) = method.function.body {
        lower_block_stmt(ctx, block)?
    } else {
        Vec::new()
    };

    ctx.exit_scope(scope_mark);

    Ok(Function {
        id: ctx.fresh_func(),
        name,
        type_params: Vec::new(),
        params,
        return_type: Type::Void,
        body,
        is_async: false,
        is_exported: false,
        captures: Vec::new(),
        decorators: Vec::new(),
    })
}

fn lower_class_prop(ctx: &mut LoweringContext, prop: &ast::ClassProp) -> Result<ClassField> {
    let name = match &prop.key {
        ast::PropName::Ident(ident) => ident.sym.to_string(),
        ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
        _ => return Err(anyhow!("Unsupported property key")),
    };

    // Extract type from type annotation (using context for class type param resolution)
    let ty = prop.type_ann.as_ref()
        .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
        .unwrap_or(Type::Any);

    // Lower initializer expression if present
    let init = prop.value.as_ref()
        .map(|e| lower_expr(ctx, e))
        .transpose()?;

    Ok(ClassField {
        name,
        ty,
        init,
        is_private: false, // TODO: check accessibility
        is_readonly: prop.readonly,
    })
}

fn lower_private_prop(ctx: &mut LoweringContext, prop: &ast::PrivateProp) -> Result<ClassField> {
    // Private fields use PrivateName which has a `name` field (without the # prefix in SWC)
    // We store the name with the # prefix to distinguish private fields
    let name = format!("#{}", prop.key.name.to_string());

    // Extract type from type annotation (using context for class type param resolution)
    let ty = prop.type_ann.as_ref()
        .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
        .unwrap_or(Type::Any);

    // Lower initializer expression if present
    let init = prop.value.as_ref()
        .map(|e| lower_expr(ctx, e))
        .transpose()?;

    Ok(ClassField {
        name,
        ty,
        init,
        is_private: true,
        is_readonly: prop.readonly,
    })
}

fn lower_block_stmt(ctx: &mut LoweringContext, block: &ast::BlockStmt) -> Result<Vec<Stmt>> {
    let mut stmts = Vec::new();
    for stmt in &block.stmts {
        stmts.extend(lower_body_stmt(ctx, stmt)?);
    }
    Ok(stmts)
}

fn lower_body_stmt(ctx: &mut LoweringContext, stmt: &ast::Stmt) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    match stmt {
        ast::Stmt::Return(ret) => {
            let value = ret.arg.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            result.push(Stmt::Return(value));
        }
        ast::Stmt::If(if_stmt) => {
            let condition = lower_expr(ctx, &if_stmt.test)?;
            let then_branch = lower_body_stmt(ctx, &if_stmt.cons)?;
            let else_branch = if_stmt.alt.as_ref()
                .map(|s| lower_body_stmt(ctx, s))
                .transpose()?;
            result.push(Stmt::If {
                condition,
                then_branch,
                else_branch,
            });
        }
        ast::Stmt::Block(block) => {
            result.extend(lower_block_stmt(ctx, block)?);
        }
        ast::Stmt::Expr(expr_stmt) => {
            // Check if this is a destructuring assignment that needs special handling
            if let ast::Expr::Assign(assign) = expr_stmt.expr.as_ref() {
                if let ast::AssignTarget::Pat(pat) = &assign.left {
                    // This is a destructuring assignment at statement level
                    // We can emit proper Let statements for temporaries
                    let stmts = lower_destructuring_assignment_stmt(ctx, pat, &assign.right)?;
                    result.extend(stmts);
                    return Ok(result);
                }
            }
            let expr = lower_expr(ctx, &expr_stmt.expr)?;
            result.push(Stmt::Expr(expr));
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            let mutable = var_decl.kind != ast::VarDeclKind::Const;
            for decl in &var_decl.decls {
                let stmts = lower_var_decl_with_destructuring(ctx, decl, mutable)?;
                result.extend(stmts);
            }
        }
        ast::Stmt::While(while_stmt) => {
            let condition = lower_expr(ctx, &while_stmt.test)?;
            let body = lower_body_stmt(ctx, &while_stmt.body)?;
            result.push(Stmt::While { condition, body });
        }
        ast::Stmt::Break(_) => {
            result.push(Stmt::Break);
        }
        ast::Stmt::Continue(_) => {
            result.push(Stmt::Continue);
        }
        ast::Stmt::For(for_stmt) => {
            // Lower the init part (can be a variable declaration or expression)
            let init = if let Some(init) = &for_stmt.init {
                match init {
                    ast::VarDeclOrExpr::VarDecl(var_decl) => {
                        // For loop initializers typically have one declaration
                        if let Some(decl) = var_decl.decls.first() {
                            let name = get_binding_name(&decl.name)?;
                            let init_expr = decl.init.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
                            let id = ctx.define_local(name.clone(), Type::Any);
                            Some(Box::new(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: true,
                                init: init_expr,
                            }))
                        } else {
                            None
                        }
                    }
                    ast::VarDeclOrExpr::Expr(expr) => {
                        Some(Box::new(Stmt::Expr(lower_expr(ctx, expr)?)))
                    }
                }
            } else {
                None
            };

            let condition = for_stmt.test.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let update = for_stmt.update.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let body = lower_body_stmt(ctx, &for_stmt.body)?;

            result.push(Stmt::For { init, condition, update, body });
        }
        ast::Stmt::Try(try_stmt) => {
            // Lower try body
            let body = lower_block_stmt(ctx, &try_stmt.block)?;

            // Lower catch clause (if present)
            let catch = if let Some(ref catch_clause) = try_stmt.handler {
                let scope_mark = ctx.enter_scope();

                // Lower catch parameter (if present)
                let param = if let Some(ref pat) = catch_clause.param {
                    let param_name = get_pat_name(pat)?;
                    let param_id = ctx.define_local(param_name.clone(), Type::Any);
                    Some((param_id, param_name))
                } else {
                    None
                };

                // Lower catch body
                let catch_body = lower_block_stmt(ctx, &catch_clause.body)?;

                ctx.exit_scope(scope_mark);

                Some(CatchClause { param, body: catch_body })
            } else {
                None
            };

            // Lower finally (if present)
            let finally = if let Some(ref finally_block) = try_stmt.finalizer {
                Some(lower_block_stmt(ctx, finally_block)?)
            } else {
                None
            };

            result.push(Stmt::Try { body, catch, finally });
        }
        ast::Stmt::Throw(throw_stmt) => {
            let expr = lower_expr(ctx, &throw_stmt.arg)?;
            result.push(Stmt::Throw(expr));
        }
        ast::Stmt::Switch(switch_stmt) => {
            let discriminant = lower_expr(ctx, &switch_stmt.discriminant)?;
            let mut cases = Vec::new();

            for case in &switch_stmt.cases {
                let test = case.test.as_ref()
                    .map(|e| lower_expr(ctx, e))
                    .transpose()?;

                let mut body = Vec::new();
                for stmt in &case.cons {
                    body.extend(lower_body_stmt(ctx, stmt)?);
                }

                cases.push(SwitchCase { test, body });
            }

            result.push(Stmt::Switch { discriminant, cases });
        }
        ast::Stmt::ForOf(for_of_stmt) => {
            // Desugar for-of to a regular for loop (same as in lower_stmt)
            let arr_expr = lower_expr(ctx, &for_of_stmt.right)?;
            let arr_id = ctx.fresh_local();
            let idx_id = ctx.fresh_local();
            ctx.locals.push((format!("__arr_{}", arr_id), arr_id, Type::Array(Box::new(Type::Any))));
            ctx.locals.push((format!("__idx_{}", idx_id), idx_id, Type::Number));

            // Store array reference
            result.push(Stmt::Let {
                id: arr_id,
                name: format!("__arr_{}", arr_id),
                ty: Type::Array(Box::new(Type::Any)),
                mutable: false,
                init: Some(arr_expr),
            });

            // IMPORTANT: Define iteration variables BEFORE lowering the body
            let item_id = ctx.fresh_local();
            ctx.locals.push((format!("__item_{}", item_id), item_id, Type::Any));

            // Pre-define all variables from the pattern
            let var_ids: Vec<(String, u32)> = match &for_of_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        match &decl.name {
                            ast::Pat::Ident(ident) => {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Any);
                                vec![(name, id)]
                            }
                            ast::Pat::Array(arr_pat) => {
                                let mut ids = Vec::new();
                                for elem in &arr_pat.elems {
                                    if let Some(elem_pat) = elem {
                                        if let ast::Pat::Ident(ident) = elem_pat {
                                            let name = ident.id.sym.to_string();
                                            let id = ctx.define_local(name.clone(), Type::Any);
                                            ids.push((name, id));
                                        }
                                    }
                                }
                                ids
                            }
                            ast::Pat::Object(obj_pat) => {
                                let mut ids = Vec::new();
                                for prop in &obj_pat.props {
                                    match prop {
                                        ast::ObjectPatProp::Assign(assign) => {
                                            let name = assign.key.sym.to_string();
                                            let id = ctx.define_local(name.clone(), Type::Any);
                                            ids.push((name, id));
                                        }
                                        ast::ObjectPatProp::KeyValue(kv) => {
                                            if let ast::Pat::Ident(ident) = &*kv.value {
                                                let name = ident.id.sym.to_string();
                                                let id = ctx.define_local(name.clone(), Type::Any);
                                                ids.push((name, id));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ids
                            }
                            _ => {
                                let name = get_binding_name(&decl.name)?;
                                let id = ctx.define_local(name.clone(), Type::Any);
                                vec![(name, id)]
                            }
                        }
                    } else {
                        return Err(anyhow!("for-of requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(pat) => {
                    let name = get_pat_name(pat)?;
                    let id = ctx.define_local(name.clone(), Type::Any);
                    vec![(name, id)]
                }
                _ => return Err(anyhow!("Unsupported for-of left-hand side")),
            };

            // NOW lower the body
            let mut loop_body = lower_body_stmt(ctx, &for_of_stmt.body)?;

            // Build binding statements using pre-defined variable IDs
            let binding_stmts = match &for_of_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        let item_expr = Expr::IndexGet {
                            object: Box::new(Expr::LocalGet(arr_id)),
                            index: Box::new(Expr::LocalGet(idx_id)),
                        };

                        match &decl.name {
                            ast::Pat::Ident(_) => {
                                let (name, id) = var_ids[0].clone();
                                vec![Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }]
                            }
                            ast::Pat::Array(arr_pat) => {
                                let mut stmts = vec![Stmt::Let {
                                    id: item_id,
                                    name: format!("__item_{}", item_id),
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }];
                                let mut var_idx = 0;
                                for (idx, elem) in arr_pat.elems.iter().enumerate() {
                                    if let Some(elem_pat) = elem {
                                        if let ast::Pat::Ident(_) = elem_pat {
                                            let (name, id) = var_ids[var_idx].clone();
                                            var_idx += 1;
                                            stmts.push(Stmt::Let {
                                                id,
                                                name,
                                                ty: Type::Any,
                                                mutable: false,
                                                init: Some(Expr::IndexGet {
                                                    object: Box::new(Expr::LocalGet(item_id)),
                                                    index: Box::new(Expr::Number(idx as f64)),
                                                }),
                                            });
                                        }
                                    }
                                }
                                stmts
                            }
                            ast::Pat::Object(obj_pat) => {
                                let mut stmts = vec![Stmt::Let {
                                    id: item_id,
                                    name: format!("__item_{}", item_id),
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(item_expr),
                                }];
                                let mut var_idx = 0;
                                for prop in &obj_pat.props {
                                    match prop {
                                        ast::ObjectPatProp::Assign(assign) => {
                                            let prop_name = assign.key.sym.to_string();
                                            let (name, id) = var_ids[var_idx].clone();
                                            var_idx += 1;
                                            stmts.push(Stmt::Let {
                                                id,
                                                name,
                                                ty: Type::Any,
                                                mutable: false,
                                                init: Some(Expr::PropertyGet {
                                                    object: Box::new(Expr::LocalGet(item_id)),
                                                    property: prop_name,
                                                }),
                                            });
                                        }
                                        ast::ObjectPatProp::KeyValue(kv) => {
                                            let key = match &kv.key {
                                                ast::PropName::Ident(ident) => ident.sym.to_string(),
                                                _ => continue,
                                            };
                                            if let ast::Pat::Ident(_) = &*kv.value {
                                                let (name, id) = var_ids[var_idx].clone();
                                                var_idx += 1;
                                                stmts.push(Stmt::Let {
                                                    id,
                                                    name,
                                                    ty: Type::Any,
                                                    mutable: false,
                                                    init: Some(Expr::PropertyGet {
                                                        object: Box::new(Expr::LocalGet(item_id)),
                                                        property: key,
                                                    }),
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                stmts
                            }
                            _ => {
                                let (name, id) = var_ids[0].clone();
                                vec![Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(Expr::IndexGet {
                                        object: Box::new(Expr::LocalGet(arr_id)),
                                        index: Box::new(Expr::LocalGet(idx_id)),
                                    }),
                                }]
                            }
                        }
                    } else {
                        return Err(anyhow!("for-of requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(_) => {
                    let (name, id) = var_ids[0].clone();
                    vec![Stmt::Let {
                        id,
                        name,
                        ty: Type::Any,
                        mutable: false,
                        init: Some(Expr::IndexGet {
                            object: Box::new(Expr::LocalGet(arr_id)),
                            index: Box::new(Expr::LocalGet(idx_id)),
                        }),
                    }]
                }
                _ => return Err(anyhow!("Unsupported for-of left-hand side")),
            };

            // Prepend the binding statements to the loop body
            for (i, stmt) in binding_stmts.into_iter().enumerate() {
                loop_body.insert(i, stmt);
            }

            // Create the for loop
            result.push(Stmt::For {
                init: Some(Box::new(Stmt::Let {
                    id: idx_id,
                    name: format!("__idx_{}", idx_id),
                    ty: Type::Number,
                    mutable: true,
                    init: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Compare {
                    op: CompareOp::Lt,
                    left: Box::new(Expr::LocalGet(idx_id)),
                    right: Box::new(Expr::PropertyGet {
                        object: Box::new(Expr::LocalGet(arr_id)),
                        property: "length".to_string(),
                    }),
                }),
                update: Some(Expr::Update {
                    id: idx_id,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
                body: loop_body,
            });
        }
        ast::Stmt::ForIn(for_in_stmt) => {
            // Desugar for-in to a for-of over Object.keys(obj) (same as in lower_stmt)
            let key_name = match &for_in_stmt.left {
                ast::ForHead::VarDecl(var_decl) => {
                    if let Some(decl) = var_decl.decls.first() {
                        get_binding_name(&decl.name)?
                    } else {
                        return Err(anyhow!("for-in requires a variable declaration"));
                    }
                }
                ast::ForHead::Pat(pat) => get_pat_name(pat)?,
                _ => return Err(anyhow!("Unsupported for-in left-hand side")),
            };

            let obj_expr = lower_expr(ctx, &for_in_stmt.right)?;
            let keys_expr = Expr::ObjectKeys(Box::new(obj_expr));
            let keys_id = ctx.fresh_local();
            let idx_id = ctx.fresh_local();
            let key_id = ctx.define_local(key_name.clone(), Type::String);

            // Store keys array reference
            result.push(Stmt::Let {
                id: keys_id,
                name: format!("__keys_{}", keys_id),
                ty: Type::Array(Box::new(Type::String)),
                mutable: false,
                init: Some(keys_expr),
            });

            // Lower the body and prepend key assignment
            let mut loop_body = lower_body_stmt(ctx, &for_in_stmt.body)?;
            loop_body.insert(0, Stmt::Let {
                id: key_id,
                name: key_name,
                ty: Type::String,
                mutable: false,
                init: Some(Expr::IndexGet {
                    object: Box::new(Expr::LocalGet(keys_id)),
                    index: Box::new(Expr::LocalGet(idx_id)),
                }),
            });

            // Create the for loop
            result.push(Stmt::For {
                init: Some(Box::new(Stmt::Let {
                    id: idx_id,
                    name: format!("__idx_{}", idx_id),
                    ty: Type::Number,
                    mutable: true,
                    init: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Compare {
                    op: CompareOp::Lt,
                    left: Box::new(Expr::LocalGet(idx_id)),
                    right: Box::new(Expr::PropertyGet {
                        object: Box::new(Expr::LocalGet(keys_id)),
                        property: "length".to_string(),
                    }),
                }),
                update: Some(Expr::Update {
                    id: idx_id,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
                body: loop_body,
            });
        }
        _ => {
            // TODO: handle more statement types
        }
    }

    Ok(result)
}

fn lower_expr(ctx: &mut LoweringContext, expr: &ast::Expr) -> Result<Expr> {
    match expr {
        ast::Expr::Lit(lit) => lower_lit(lit),
        ast::Expr::Ident(ident) => {
            let name = ident.sym.to_string();
            if let Some(id) = ctx.lookup_local(&name) {
                Ok(Expr::LocalGet(id))
            } else if let Some(id) = ctx.lookup_func(&name) {
                Ok(Expr::FuncRef(id))
            } else if let Some((module_name, _method_name)) = ctx.lookup_native_module(&name) {
                // Native module reference (e.g., mysql from 'mysql2/promise')
                Ok(Expr::NativeModuleRef(module_name.to_string()))
            } else if let Some(orig_name) = ctx.lookup_imported_func(&name) {
                // Imported function - reference by its original exported name
                // Look up type information if available
                let (param_types, return_type) = ctx.lookup_extern_func_types(orig_name)
                    .map(|(p, r)| (p.clone(), r.clone()))
                    .unwrap_or_else(|| (Vec::new(), Type::Any));
                Ok(Expr::ExternFuncRef {
                    name: orig_name.to_string(),
                    param_types,
                    return_type,
                })
            } else if is_builtin_function(&name) {
                // Built-in global function (setTimeout, etc.)
                Ok(Expr::ExternFuncRef {
                    name,
                    param_types: Vec::new(),
                    return_type: Type::Any,
                })
            } else if name == "undefined" {
                // Global undefined identifier
                Ok(Expr::Undefined)
            } else if name == "null" {
                // Global null identifier (though typically written as literal)
                Ok(Expr::Null)
            } else if name == "NaN" {
                // Global NaN identifier
                Ok(Expr::Number(f64::NAN))
            } else if name == "Infinity" {
                // Global Infinity identifier
                Ok(Expr::Number(f64::INFINITY))
            } else {
                // Assume it's a global (like console)
                Ok(Expr::GlobalGet(0)) // TODO: proper global lookup
            }
        }
        ast::Expr::Bin(bin) => {
            // Handle 'in' operator: property in object
            if matches!(bin.op, ast::BinaryOp::In) {
                let property = Box::new(lower_expr(ctx, &bin.left)?);
                let object = Box::new(lower_expr(ctx, &bin.right)?);
                return Ok(Expr::In { property, object });
            }

            // Handle instanceof specially - needs to extract class name
            if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                let expr = Box::new(lower_expr(ctx, &bin.left)?);
                // Right side can be an identifier (ClassName) or member expression (Module.ClassName)
                let ty = match bin.right.as_ref() {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    ast::Expr::Member(member) => {
                        // Handle Module.ClassName - extract the full qualified name
                        let obj_name = if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                            obj_ident.sym.to_string()
                        } else {
                            "Unknown".to_string()
                        };
                        let prop_name = match &member.prop {
                            ast::MemberProp::Ident(prop_ident) => prop_ident.sym.to_string(),
                            _ => "Unknown".to_string(),
                        };
                        format!("{}.{}", obj_name, prop_name)
                    }
                    _ => {
                        // For complex expressions, use a generic type name
                        "Object".to_string()
                    }
                };
                return Ok(Expr::InstanceOf { expr, ty });
            }

            let left = Box::new(lower_expr(ctx, &bin.left)?);
            let right = Box::new(lower_expr(ctx, &bin.right)?);

            match bin.op {
                // Arithmetic
                ast::BinaryOp::Add => Ok(Expr::Binary { op: BinaryOp::Add, left, right }),
                ast::BinaryOp::Sub => Ok(Expr::Binary { op: BinaryOp::Sub, left, right }),
                ast::BinaryOp::Mul => Ok(Expr::Binary { op: BinaryOp::Mul, left, right }),
                ast::BinaryOp::Div => Ok(Expr::Binary { op: BinaryOp::Div, left, right }),
                ast::BinaryOp::Mod => Ok(Expr::Binary { op: BinaryOp::Mod, left, right }),
                ast::BinaryOp::Exp => Ok(Expr::Binary { op: BinaryOp::Pow, left, right }),

                // Comparison (treat == same as === for typed code)
                ast::BinaryOp::EqEq => Ok(Expr::Compare { op: CompareOp::Eq, left, right }),
                ast::BinaryOp::EqEqEq => Ok(Expr::Compare { op: CompareOp::Eq, left, right }),
                ast::BinaryOp::NotEq => Ok(Expr::Compare { op: CompareOp::Ne, left, right }),
                ast::BinaryOp::NotEqEq => Ok(Expr::Compare { op: CompareOp::Ne, left, right }),
                ast::BinaryOp::Lt => Ok(Expr::Compare { op: CompareOp::Lt, left, right }),
                ast::BinaryOp::LtEq => Ok(Expr::Compare { op: CompareOp::Le, left, right }),
                ast::BinaryOp::Gt => Ok(Expr::Compare { op: CompareOp::Gt, left, right }),
                ast::BinaryOp::GtEq => Ok(Expr::Compare { op: CompareOp::Ge, left, right }),

                // Logical
                ast::BinaryOp::LogicalAnd => Ok(Expr::Logical { op: LogicalOp::And, left, right }),
                ast::BinaryOp::LogicalOr => Ok(Expr::Logical { op: LogicalOp::Or, left, right }),
                ast::BinaryOp::NullishCoalescing => Ok(Expr::Logical { op: LogicalOp::Coalesce, left, right }),

                // Bitwise
                ast::BinaryOp::BitAnd => Ok(Expr::Binary { op: BinaryOp::BitAnd, left, right }),
                ast::BinaryOp::BitOr => Ok(Expr::Binary { op: BinaryOp::BitOr, left, right }),
                ast::BinaryOp::BitXor => Ok(Expr::Binary { op: BinaryOp::BitXor, left, right }),
                ast::BinaryOp::LShift => Ok(Expr::Binary { op: BinaryOp::Shl, left, right }),
                ast::BinaryOp::RShift => Ok(Expr::Binary { op: BinaryOp::Shr, left, right }),
                ast::BinaryOp::ZeroFillRShift => Ok(Expr::Binary { op: BinaryOp::UShr, left, right }),

                _ => Err(anyhow!("Unsupported binary operator: {:?}", bin.op)),
            }
        }
        ast::Expr::Unary(unary) => {
            let operand = Box::new(lower_expr(ctx, &unary.arg)?);
            match unary.op {
                ast::UnaryOp::Minus => Ok(Expr::Unary { op: UnaryOp::Neg, operand }),
                ast::UnaryOp::Plus => Ok(Expr::Unary { op: UnaryOp::Pos, operand }),
                ast::UnaryOp::Bang => Ok(Expr::Unary { op: UnaryOp::Not, operand }),
                ast::UnaryOp::Tilde => Ok(Expr::Unary { op: UnaryOp::BitNot, operand }),
                ast::UnaryOp::TypeOf => Ok(Expr::TypeOf(operand)),
                ast::UnaryOp::Delete => Ok(Expr::Delete(operand)),
                _ => Err(anyhow!("Unsupported unary operator: {:?}", unary.op)),
            }
        }
        ast::Expr::Call(call) => {
            // Check if any argument has spread
            let has_spread = call.args.iter().any(|arg| arg.spread.is_some());

            let mut args = call.args.iter()
                .map(|arg| lower_expr(ctx, &arg.expr))
                .collect::<Result<Vec<_>>>()?;

            // If spread is present, create CallSpread instead of Call
            let spread_args: Option<Vec<CallArg>> = if has_spread {
                Some(call.args.iter().zip(args.iter())
                    .map(|(ast_arg, lowered)| {
                        if ast_arg.spread.is_some() {
                            CallArg::Spread(lowered.clone())
                        } else {
                            CallArg::Expr(lowered.clone())
                        }
                    })
                    .collect())
            } else {
                None
            };

            match &call.callee {
                ast::Callee::Super(_) => {
                    // super() call in constructor
                    Ok(Expr::SuperCall(args))
                }
                ast::Callee::Expr(expr) => {
                    // Check for super.method() call
                    if let ast::Expr::SuperProp(super_prop) = expr.as_ref() {
                        if let ast::SuperProp::Ident(ident) = &super_prop.prop {
                            return Ok(Expr::SuperMethodCall {
                                method: ident.sym.to_string(),
                                args,
                            });
                        }
                    }

                    // Check for native module method calls (e.g., mysql.createConnection())
                    if let ast::Expr::Member(member) = expr.as_ref() {
                        if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                            let obj_name = obj_ident.sym.to_string();

                            // Check for process module methods
                            if obj_name == "process" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "uptime" => return Ok(Expr::ProcessUptime),
                                        "cwd" => return Ok(Expr::ProcessCwd),
                                        "memoryUsage" => return Ok(Expr::ProcessMemoryUsage),
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for os module methods FIRST (before generic NativeMethodCall)
                            let is_os_module = obj_name == "os" ||
                                ctx.lookup_builtin_module_alias(&obj_name) == Some("os");
                            if is_os_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "platform" => return Ok(Expr::OsPlatform),
                                        "arch" => return Ok(Expr::OsArch),
                                        "hostname" => return Ok(Expr::OsHostname),
                                        "homedir" => return Ok(Expr::OsHomedir),
                                        "tmpdir" => return Ok(Expr::OsTmpdir),
                                        "totalmem" => return Ok(Expr::OsTotalmem),
                                        "freemem" => return Ok(Expr::OsFreemem),
                                        "uptime" => return Ok(Expr::OsUptime),
                                        "type" => return Ok(Expr::OsType),
                                        "release" => return Ok(Expr::OsRelease),
                                        "cpus" => return Ok(Expr::OsCpus),
                                        "networkInterfaces" => return Ok(Expr::OsNetworkInterfaces),
                                        "userInfo" => return Ok(Expr::OsUserInfo),
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Buffer static methods
                            if obj_name == "Buffer" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "from" => {
                                            let data = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            let encoding = args.get(1).cloned().map(Box::new);
                                            return Ok(Expr::BufferFrom {
                                                data: Box::new(data),
                                                encoding
                                            });
                                        }
                                        "alloc" => {
                                            let size = args.get(0).cloned().unwrap_or(Expr::Number(0.0));
                                            let fill = args.get(1).cloned().map(Box::new);
                                            return Ok(Expr::BufferAlloc {
                                                size: Box::new(size),
                                                fill
                                            });
                                        }
                                        "allocUnsafe" => {
                                            let size = args.get(0).cloned().unwrap_or(Expr::Number(0.0));
                                            return Ok(Expr::BufferAllocUnsafe(Box::new(size)));
                                        }
                                        "concat" => {
                                            let list = args.get(0).cloned().unwrap_or(Expr::Array(vec![]));
                                            return Ok(Expr::BufferConcat(Box::new(list)));
                                        }
                                        "isBuffer" => {
                                            let obj = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::BufferIsBuffer(Box::new(obj)));
                                        }
                                        "byteLength" => {
                                            let data = args.get(0).cloned().unwrap_or(Expr::String("".to_string()));
                                            return Ok(Expr::BufferByteLength(Box::new(data)));
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Uint8Array static methods
                            if obj_name == "Uint8Array" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "from" => {
                                            let data = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::Uint8ArrayFrom(Box::new(data)));
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Object static methods
                            if obj_name == "Object" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "keys" => {
                                            let obj = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::ObjectKeys(Box::new(obj)));
                                        }
                                        "values" => {
                                            let obj = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::ObjectValues(Box::new(obj)));
                                        }
                                        "entries" => {
                                            let obj = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::ObjectEntries(Box::new(obj)));
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Array static methods
                            if obj_name == "Array" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "isArray" => {
                                            let value = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            return Ok(Expr::ArrayIsArray(Box::new(value)));
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for net module methods
                            let is_net_module = obj_name == "net" ||
                                ctx.lookup_builtin_module_alias(&obj_name) == Some("net");
                            if is_net_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "createServer" => {
                                            let options = args.get(0).cloned().map(Box::new);
                                            let connection_listener = args.get(1).cloned().map(Box::new);
                                            return Ok(Expr::NetCreateServer { options, connection_listener });
                                        }
                                        "createConnection" | "connect" => {
                                            let port = args.get(0).cloned().unwrap_or(Expr::Number(0.0));
                                            let host = args.get(1).cloned().map(Box::new);
                                            let connect_listener = args.get(2).cloned().map(Box::new);
                                            return Ok(Expr::NetCreateConnection {
                                                port: Box::new(port),
                                                host,
                                                connect_listener
                                            });
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            if let Some((module_name, _imported_method)) = ctx.lookup_native_module(&obj_name) {
                                // This is a call on a native module (e.g., mysql.createConnection)
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.to_string();
                                    return Ok(Expr::NativeMethodCall {
                                        module: module_name.to_string(),
                                        class_name: None,  // Will be set by js_transform if needed
                                        object: None,  // Static call on module itself
                                        method: method_name,
                                        args,
                                    });
                                }
                            }
                        }
                    }

                    // Check for static method calls (e.g., Counter.increment())
                    if let ast::Expr::Member(member) = expr.as_ref() {
                        if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                            let obj_name = obj_ident.sym.to_string();
                            if ctx.lookup_class(&obj_name).is_some() {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.to_string();
                                    if ctx.has_static_method(&obj_name, &method_name) {
                                        return Ok(Expr::StaticMethodCall {
                                            class_name: obj_name,
                                            method_name,
                                            args,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Check for native instance method calls (e.g., emitter.on(), ws.send())
                    if let ast::Expr::Member(member) = expr.as_ref() {
                        if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                            let obj_name = obj_ident.sym.to_string();
                            // Clone module_name and class_name to avoid borrow issues
                            let native_instance = ctx.lookup_native_instance(&obj_name)
                                .map(|(m, c)| (m.to_string(), c.to_string()));
                            if let Some((module_name, class_name)) = native_instance {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.to_string();
                                    // Get the object expression (the instance variable)
                                    let object_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::NativeMethodCall {
                                        module: module_name,
                                        class_name: Some(class_name),  // Use the registered class name
                                        object: Some(Box::new(object_expr)),
                                        method: method_name,
                                        args,
                                    });
                                }
                            }
                        }

                        // Check for method calls on new Big/Decimal/BigNumber() expressions
                        // e.g., new Big("100").div(2)
                        if let Some(module_name) = detect_native_instance_expr(&member.obj) {
                            if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                let method_name = method_ident.sym.to_string();
                                let object_expr = lower_expr(ctx, &member.obj)?;
                                return Ok(Expr::NativeMethodCall {
                                    module: module_name.to_string(),
                                    class_name: None,  // Will be set by js_transform if needed
                                    object: Some(Box::new(object_expr)),
                                    method: method_name,
                                    args,
                                });
                            }
                        }

                        // Check for chained method calls on registered native instances
                        // e.g., r1.times(...).times(...) where r1 is a Big
                        // The inner call might lower to a NativeMethodCall, and we need to chain properly
                        if let ast::MemberProp::Ident(method_ident) = &member.prop {
                            let method_name = method_ident.sym.to_string();
                            // Lower the object expression first
                            let object_expr = lower_expr(ctx, &member.obj)?;
                            // Check if it's a NativeMethodCall for a math library
                            if let Expr::NativeMethodCall { module, class_name, .. } = &object_expr {
                                // Methods that return the same type (builder pattern)
                                let is_math_lib = matches!(module.as_str(), "big.js" | "decimal.js" | "bignumber.js");
                                let is_fluent_method = matches!(method_name.as_str(),
                                    "plus" | "minus" | "times" | "div" | "mod" |
                                    "pow" | "sqrt" | "abs" | "neg" | "round" | "floor" | "ceil" | "toFixed"
                                );
                                if is_math_lib && is_fluent_method {
                                    return Ok(Expr::NativeMethodCall {
                                        module: module.clone(),
                                        class_name: class_name.clone(),
                                        object: Some(Box::new(object_expr)),
                                        method: method_name,
                                        args,
                                    });
                                }
                            }
                        }
                    }

                    // Check for fs.methodName() calls (including require('fs') aliases)
                    if let ast::Expr::Member(member) = expr.as_ref() {
                        if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                            // Check if this is 'fs' directly or an alias from require('fs')
                            let obj_name = obj_ident.sym.as_ref();
                            let is_fs_module = obj_name == "fs" ||
                                ctx.lookup_builtin_module_alias(obj_name) == Some("fs");
                            if is_fs_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "readFileSync" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::FsReadFileSync(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "writeFileSync" => {
                                            if args.len() >= 2 {
                                                let mut iter = args.into_iter();
                                                let path = iter.next().unwrap();
                                                let content = iter.next().unwrap();
                                                return Ok(Expr::FsWriteFileSync(Box::new(path), Box::new(content)));
                                            }
                                        }
                                        "existsSync" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::FsExistsSync(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "mkdirSync" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::FsMkdirSync(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "unlinkSync" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::FsUnlinkSync(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for path.methodName() calls (including require('path') aliases)
                            let is_path_module = obj_name == "path" ||
                                ctx.lookup_builtin_module_alias(obj_name) == Some("path");
                            if is_path_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "join" => {
                                            if args.len() >= 2 {
                                                let mut iter = args.into_iter();
                                                let a = iter.next().unwrap();
                                                let b = iter.next().unwrap();
                                                return Ok(Expr::PathJoin(Box::new(a), Box::new(b)));
                                            }
                                        }
                                        "dirname" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::PathDirname(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "basename" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::PathBasename(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "extname" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::PathExtname(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "resolve" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::PathResolve(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for JSON.methodName() calls
                            if obj_ident.sym.as_ref() == "JSON" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "parse" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::JsonParse(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "stringify" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::JsonStringify(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Math.methodName() calls
                            if obj_ident.sym.as_ref() == "Math" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "floor" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::MathFloor(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "ceil" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::MathCeil(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "round" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::MathRound(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "abs" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::MathAbs(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "sqrt" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::MathSqrt(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "pow" => {
                                            if args.len() >= 2 {
                                                let mut args_iter = args.into_iter();
                                                let base = args_iter.next().unwrap();
                                                let exp = args_iter.next().unwrap();
                                                return Ok(Expr::MathPow(Box::new(base), Box::new(exp)));
                                            }
                                        }
                                        "min" => {
                                            return Ok(Expr::MathMin(args));
                                        }
                                        "max" => {
                                            return Ok(Expr::MathMax(args));
                                        }
                                        "random" => {
                                            return Ok(Expr::MathRandom);
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for String.methodName() static calls
                            if obj_ident.sym.as_ref() == "String" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "fromCharCode" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::StringFromCharCode(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for crypto.methodName() calls (including require('crypto') aliases)
                            let is_crypto_module = obj_name == "crypto" ||
                                ctx.lookup_builtin_module_alias(obj_name) == Some("crypto");
                            if is_crypto_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "randomBytes" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::CryptoRandomBytes(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "randomUUID" => {
                                            return Ok(Expr::CryptoRandomUUID);
                                        }
                                        "sha256" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::CryptoSha256(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "md5" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::CryptoMd5(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for os.methodName() calls (including require('os') aliases)
                            let is_os_module = obj_name == "os" ||
                                ctx.lookup_builtin_module_alias(obj_name) == Some("os");
                            if is_os_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "platform" => {
                                            return Ok(Expr::OsPlatform);
                                        }
                                        "arch" => {
                                            return Ok(Expr::OsArch);
                                        }
                                        "hostname" => {
                                            return Ok(Expr::OsHostname);
                                        }
                                        "homedir" => {
                                            return Ok(Expr::OsHomedir);
                                        }
                                        "tmpdir" => {
                                            return Ok(Expr::OsTmpdir);
                                        }
                                        "totalmem" => {
                                            return Ok(Expr::OsTotalmem);
                                        }
                                        "freemem" => {
                                            return Ok(Expr::OsFreemem);
                                        }
                                        "uptime" => {
                                            return Ok(Expr::OsUptime);
                                        }
                                        "type" => {
                                            return Ok(Expr::OsType);
                                        }
                                        "release" => {
                                            return Ok(Expr::OsRelease);
                                        }
                                        "cpus" => {
                                            return Ok(Expr::OsCpus);
                                        }
                                        "networkInterfaces" => {
                                            return Ok(Expr::OsNetworkInterfaces);
                                        }
                                        "userInfo" => {
                                            return Ok(Expr::OsUserInfo);
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Buffer.methodName() static calls
                            if obj_name == "Buffer" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "from" => {
                                            let data = args.get(0).cloned().unwrap_or(Expr::Undefined);
                                            let encoding = args.get(1).cloned().map(Box::new);
                                            return Ok(Expr::BufferFrom {
                                                data: Box::new(data),
                                                encoding,
                                            });
                                        }
                                        "alloc" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let size = args_iter.next().unwrap();
                                                let fill = args_iter.next().map(Box::new);
                                                return Ok(Expr::BufferAlloc {
                                                    size: Box::new(size),
                                                    fill,
                                                });
                                            }
                                        }
                                        "allocUnsafe" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::BufferAllocUnsafe(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "concat" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::BufferConcat(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "isBuffer" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::BufferIsBuffer(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        "byteLength" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::BufferByteLength(Box::new(args.into_iter().next().unwrap())));
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for child_process named imports (execSync, spawnSync, spawn, exec)
                            let is_child_process_module = ctx.lookup_builtin_module_alias(obj_name) == Some("child_process");
                            if is_child_process_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "execSync" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let command = args_iter.next().unwrap();
                                                let options = args_iter.next().map(Box::new);
                                                return Ok(Expr::ChildProcessExecSync {
                                                    command: Box::new(command),
                                                    options,
                                                });
                                            }
                                        }
                                        "spawnSync" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let command = args_iter.next().unwrap();
                                                let spawn_args = args_iter.next().map(Box::new);
                                                let options = args_iter.next().map(Box::new);
                                                return Ok(Expr::ChildProcessSpawnSync {
                                                    command: Box::new(command),
                                                    args: spawn_args,
                                                    options,
                                                });
                                            }
                                        }
                                        "spawn" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let command = args_iter.next().unwrap();
                                                let spawn_args = args_iter.next().map(Box::new);
                                                let options = args_iter.next().map(Box::new);
                                                return Ok(Expr::ChildProcessSpawn {
                                                    command: Box::new(command),
                                                    args: spawn_args,
                                                    options,
                                                });
                                            }
                                        }
                                        "exec" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let command = args_iter.next().unwrap();
                                                let options = args_iter.next().map(Box::new);
                                                let callback = args_iter.next().map(Box::new);
                                                return Ok(Expr::ChildProcessExec {
                                                    command: Box::new(command),
                                                    options,
                                                    callback,
                                                });
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for net.methodName() calls
                            let is_net_module = obj_name == "net" ||
                                ctx.lookup_builtin_module_alias(obj_name) == Some("net");
                            if is_net_module {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    match method_name {
                                        "createServer" => {
                                            let mut args_iter = args.into_iter();
                                            let options = args_iter.next().map(Box::new);
                                            let connection_listener = args_iter.next().map(Box::new);
                                            return Ok(Expr::NetCreateServer {
                                                options,
                                                connection_listener,
                                            });
                                        }
                                        "createConnection" | "connect" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let port = args_iter.next().unwrap();
                                                let host = args_iter.next().map(Box::new);
                                                let connect_listener = args_iter.next().map(Box::new);
                                                return Ok(Expr::NetCreateConnection {
                                                    port: Box::new(port),
                                                    host,
                                                    connect_listener,
                                                });
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }
                                }
                            }

                            // Check for Date.now() static method call
                            if obj_ident.sym.as_ref() == "Date" {
                                if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    if method_name == "now" {
                                        return Ok(Expr::DateNow);
                                    }
                                }
                            }
                        }

                        // Check for Date instance method calls (date.getTime(), etc.)
                        if let ast::MemberProp::Ident(method_ident) = &member.prop {
                            let method_name = method_ident.sym.as_ref();
                            match method_name {
                                "getTime" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetTime(Box::new(date_expr)));
                                }
                                "toISOString" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateToISOString(Box::new(date_expr)));
                                }
                                "getFullYear" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetFullYear(Box::new(date_expr)));
                                }
                                "getMonth" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetMonth(Box::new(date_expr)));
                                }
                                "getDate" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetDate(Box::new(date_expr)));
                                }
                                "getHours" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetHours(Box::new(date_expr)));
                                }
                                "getMinutes" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetMinutes(Box::new(date_expr)));
                                }
                                "getSeconds" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetSeconds(Box::new(date_expr)));
                                }
                                "getMilliseconds" => {
                                    let date_expr = lower_expr(ctx, &member.obj)?;
                                    return Ok(Expr::DateGetMilliseconds(Box::new(date_expr)));
                                }
                                _ => {} // Fall through to other handling
                            }
                        }

                        // Check for array method calls (arr.push, arr.pop, etc.)
                        // These are called on local variables, not global modules
                        // IMPORTANT: Only apply to actual Array types, not String types
                        if let ast::MemberProp::Ident(method_ident) = &member.prop {
                            let method_name = method_ident.sym.as_ref();
                            if let ast::Expr::Ident(arr_ident) = member.obj.as_ref() {
                                let arr_name = arr_ident.sym.to_string();
                                // Check that this is NOT a String type (Array, Set, Map are all OK)
                                // When type is unknown, only enter array block for array-only methods
                                // (push, pop, etc.), NOT for methods shared with strings (indexOf,
                                // includes, split) — those are handled by the general dispatch which
                                // checks is_string at codegen time.
                                let type_info = ctx.lookup_local_type(&arr_name);
                                let is_known_string = type_info.map(|ty| matches!(ty, Type::String)).unwrap_or(false);
                                let is_known_not_string = type_info.map(|ty| !matches!(ty, Type::String | Type::Any | Type::Unknown)).unwrap_or(false);
                                let is_ambiguous_method = matches!(method_name,
                                    "indexOf" | "includes" | "slice"
                                );
                                let is_not_string = if is_known_string {
                                    false  // definitely a string, skip array block
                                } else if is_known_not_string {
                                    true   // definitely not a string, enter array block
                                } else if is_ambiguous_method {
                                    false  // type unknown + ambiguous method, skip array block (fall through to general dispatch)
                                } else {
                                    true   // type unknown + array-only method (push, pop, etc.), enter array block
                                };
                                if is_not_string {
                                if let Some(array_id) = ctx.lookup_local(&arr_name) {
                                    match method_name {
                                        "push" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayPush {
                                                    array_id,
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "pop" => {
                                            return Ok(Expr::ArrayPop(array_id));
                                        }
                                        "shift" => {
                                            return Ok(Expr::ArrayShift(array_id));
                                        }
                                        "unshift" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayUnshift {
                                                    array_id,
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "indexOf" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayIndexOf {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "includes" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayIncludes {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "slice" => {
                                            // arr.slice(start, end?) - returns new array
                                            // Only convert to ArraySlice if we KNOW it's an Array type
                                            // (Type::Any could be a string, which has its own .slice() method)
                                            let is_definitely_array = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Array(_)))
                                                .unwrap_or(false);
                                            if is_definitely_array && args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let start = args_iter.next().unwrap();
                                                let end = args_iter.next();
                                                return Ok(Expr::ArraySlice {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    start: Box::new(start),
                                                    end: end.map(Box::new),
                                                });
                                            }
                                            // Fall through to normal Call handling for strings or unknown types
                                        }
                                        "splice" => {
                                            // arr.splice(start, deleteCount?, ...items) - returns deleted elements
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let start = args_iter.next().unwrap();
                                                let delete_count = args_iter.next();
                                                let items: Vec<Expr> = args_iter.collect();
                                                return Ok(Expr::ArraySplice {
                                                    array_id,
                                                    start: Box::new(start),
                                                    delete_count: delete_count.map(Box::new),
                                                    items,
                                                });
                                            }
                                        }
                                        "forEach" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayForEach {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "map" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayMap {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "filter" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayFilter {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "find" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayFind {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "findIndex" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayFindIndex {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "reduce" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let callback = args_iter.next().unwrap();
                                                let initial = args_iter.next().map(Box::new);
                                                return Ok(Expr::ArrayReduce {
                                                    array: Box::new(Expr::LocalGet(array_id)),
                                                    callback: Box::new(callback),
                                                    initial,
                                                });
                                            }
                                        }
                                        "join" => {
                                            // arr.join(separator?) -> string
                                            let separator = args.into_iter().next().map(Box::new);
                                            return Ok(Expr::ArrayJoin {
                                                array: Box::new(Expr::LocalGet(array_id)),
                                                separator,
                                            });
                                        }
                                        // Map methods (only apply to actual Map/Set types)
                                        "set" => {
                                            // Check if this is a Map or Set type before treating as Map.set()
                                            let is_map_or_set = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Map" || base == "Set"))
                                                .unwrap_or(false);
                                            if is_map_or_set && args.len() >= 2 {
                                                // map.set(key, value) - returns the map for chaining
                                                let mut args_iter = args.into_iter();
                                                let key = args_iter.next().unwrap();
                                                let value = args_iter.next().unwrap();
                                                return Ok(Expr::MapSet {
                                                    map: Box::new(Expr::LocalGet(array_id)),
                                                    key: Box::new(key),
                                                    value: Box::new(value),
                                                });
                                            }
                                        }
                                        "get" => {
                                            // Check if this is a Map type before treating as Map.get()
                                            let is_map = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Map"))
                                                .unwrap_or(false);
                                            if is_map && args.len() >= 1 {
                                                // map.get(key) - returns value or undefined
                                                return Ok(Expr::MapGet {
                                                    map: Box::new(Expr::LocalGet(array_id)),
                                                    key: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "has" => {
                                            // Check if this is a Set or Map - only apply to actual Set/Map types
                                            let is_set = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Set"))
                                                .unwrap_or(false);
                                            let is_map = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Map"))
                                                .unwrap_or(false);
                                            if (is_set || is_map) && args.len() >= 1 {
                                                let value = args.into_iter().next().unwrap();
                                                if is_set {
                                                    return Ok(Expr::SetHas {
                                                        set: Box::new(Expr::LocalGet(array_id)),
                                                        value: Box::new(value),
                                                    });
                                                } else {
                                                    return Ok(Expr::MapHas {
                                                        map: Box::new(Expr::LocalGet(array_id)),
                                                        key: Box::new(value),
                                                    });
                                                }
                                            }
                                        }
                                        "delete" => {
                                            // Check if this is a Set or Map - only apply to actual Set/Map types
                                            let is_set = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Set"))
                                                .unwrap_or(false);
                                            let is_map = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Map"))
                                                .unwrap_or(false);
                                            if (is_set || is_map) && args.len() >= 1 {
                                                let value = args.into_iter().next().unwrap();
                                                if is_set {
                                                    return Ok(Expr::SetDelete {
                                                        set: Box::new(Expr::LocalGet(array_id)),
                                                        value: Box::new(value),
                                                    });
                                                } else {
                                                    return Ok(Expr::MapDelete {
                                                        map: Box::new(Expr::LocalGet(array_id)),
                                                        key: Box::new(value),
                                                    });
                                                }
                                            }
                                        }
                                        "clear" => {
                                            // Check if this is a Set or Map - only apply to actual Set/Map types
                                            let is_set = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Set"))
                                                .unwrap_or(false);
                                            let is_map = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Map"))
                                                .unwrap_or(false);
                                            if is_set {
                                                return Ok(Expr::SetClear(Box::new(Expr::LocalGet(array_id))));
                                            } else if is_map {
                                                return Ok(Expr::MapClear(Box::new(Expr::LocalGet(array_id))));
                                            }
                                            // Fall through if neither Set nor Map
                                        }
                                        // Set methods
                                        "add" => {
                                            // Check if this is a Set type before treating as Set.add()
                                            let is_set = ctx.lookup_local_type(&arr_name)
                                                .map(|ty| matches!(ty, Type::Generic { base, .. } if base == "Set"))
                                                .unwrap_or(false);
                                            if is_set && args.len() >= 1 {
                                                // set.add(value) - returns the set for chaining
                                                let value = args.into_iter().next().unwrap();
                                                return Ok(Expr::SetAdd {
                                                    set_id: array_id,
                                                    value: Box::new(value),
                                                });
                                            }
                                        }
                                        _ => {} // Fall through to generic handling
                                    }

                                    // URLSearchParams methods
                                    let is_url_search_params = ctx.lookup_local_type(&arr_name)
                                        .map(|ty| matches!(ty, Type::Named(name) if name == "URLSearchParams"))
                                        .unwrap_or(false);
                                    if is_url_search_params {
                                        match method_name {
                                            "get" => {
                                                if args.len() >= 1 {
                                                    return Ok(Expr::UrlSearchParamsGet {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(args.into_iter().next().unwrap()),
                                                    });
                                                }
                                            }
                                            "has" => {
                                                if args.len() >= 1 {
                                                    return Ok(Expr::UrlSearchParamsHas {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(args.into_iter().next().unwrap()),
                                                    });
                                                }
                                            }
                                            "set" => {
                                                if args.len() >= 2 {
                                                    let mut args_iter = args.into_iter();
                                                    let name_arg = args_iter.next().unwrap();
                                                    let value_arg = args_iter.next().unwrap();
                                                    return Ok(Expr::UrlSearchParamsSet {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(name_arg),
                                                        value: Box::new(value_arg),
                                                    });
                                                }
                                            }
                                            "append" => {
                                                if args.len() >= 2 {
                                                    let mut args_iter = args.into_iter();
                                                    let name_arg = args_iter.next().unwrap();
                                                    let value_arg = args_iter.next().unwrap();
                                                    return Ok(Expr::UrlSearchParamsAppend {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(name_arg),
                                                        value: Box::new(value_arg),
                                                    });
                                                }
                                            }
                                            "delete" => {
                                                if args.len() >= 1 {
                                                    return Ok(Expr::UrlSearchParamsDelete {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(args.into_iter().next().unwrap()),
                                                    });
                                                }
                                            }
                                            "toString" => {
                                                return Ok(Expr::UrlSearchParamsToString(
                                                    Box::new(Expr::LocalGet(array_id))
                                                ));
                                            }
                                            "getAll" => {
                                                if args.len() >= 1 {
                                                    return Ok(Expr::UrlSearchParamsGetAll {
                                                        params: Box::new(Expr::LocalGet(array_id)),
                                                        name: Box::new(args.into_iter().next().unwrap()),
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                }  // close is_array_type check
                            }
                        }
                    }

                    // Check for array methods on imported variables (e.g., import { CHAIN_NAMES } from './module')
                    // These don't have local IDs but are ExternFuncRef values
                    if let ast::Callee::Expr(expr) = &call.callee {
                        if let ast::Expr::Member(member) = expr.as_ref() {
                            if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                let method_name = method_ident.sym.as_ref();
                                if let ast::Expr::Ident(arr_ident) = member.obj.as_ref() {
                                    let arr_name = arr_ident.sym.to_string();
                                    // Check if this is an imported variable (not a local)
                                    if ctx.lookup_local(&arr_name).is_none() {
                                        if let Some(orig_name) = ctx.lookup_imported_func(&arr_name) {
                                            // This is an imported variable - create ExternFuncRef for it
                                            let (param_types, return_type) = ctx.lookup_extern_func_types(orig_name)
                                                .map(|(p, r)| (p.clone(), r.clone()))
                                                .unwrap_or_else(|| (Vec::new(), Type::Any));
                                            let extern_ref = Expr::ExternFuncRef {
                                                name: orig_name.to_string(),
                                                param_types,
                                                return_type,
                                            };
                                            match method_name {
                                                "join" => {
                                                    // arr.join(separator?) -> string
                                                    let separator = args.into_iter().next().map(Box::new);
                                                    return Ok(Expr::ArrayJoin {
                                                        array: Box::new(extern_ref),
                                                        separator,
                                                    });
                                                }
                                                "map" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayMap {
                                                            array: Box::new(extern_ref),
                                                            callback: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "filter" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayFilter {
                                                            array: Box::new(extern_ref),
                                                            callback: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "forEach" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayForEach {
                                                            array: Box::new(extern_ref),
                                                            callback: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "find" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayFind {
                                                            array: Box::new(extern_ref),
                                                            callback: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "indexOf" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayIndexOf {
                                                            array: Box::new(extern_ref),
                                                            value: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "includes" => {
                                                    if args.len() >= 1 {
                                                        return Ok(Expr::ArrayIncludes {
                                                            array: Box::new(extern_ref),
                                                            value: Box::new(args.into_iter().next().unwrap()),
                                                        });
                                                    }
                                                }
                                                "slice" => {
                                                    if args.len() >= 1 {
                                                        let mut args_iter = args.into_iter();
                                                        let start = args_iter.next().unwrap();
                                                        let end = args_iter.next();
                                                        return Ok(Expr::ArraySlice {
                                                            array: Box::new(extern_ref),
                                                            start: Box::new(start),
                                                            end: end.map(Box::new),
                                                        });
                                                    }
                                                }
                                                "reduce" => {
                                                    if args.len() >= 1 {
                                                        let mut args_iter = args.into_iter();
                                                        let callback = args_iter.next().unwrap();
                                                        let initial = args_iter.next().map(Box::new);
                                                        return Ok(Expr::ArrayReduce {
                                                            array: Box::new(extern_ref),
                                                            callback: Box::new(callback),
                                                            initial,
                                                        });
                                                    }
                                                }
                                                _ => {} // Fall through for other methods
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check for array methods on inline array literals (e.g., ['a', 'b'].join('-'))
                    if let ast::Callee::Expr(expr) = &call.callee {
                        if let ast::Expr::Member(member) = expr.as_ref() {
                            if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                let method_name = method_ident.sym.as_ref();
                                if let ast::Expr::Array(_arr_lit) = member.obj.as_ref() {
                                    // Lower the array literal
                                    let array_expr = lower_expr(ctx, &member.obj)?;
                                    match method_name {
                                        "join" => {
                                            // ['a', 'b'].join(separator?) -> string
                                            let separator = args.into_iter().next().map(Box::new);
                                            return Ok(Expr::ArrayJoin {
                                                array: Box::new(array_expr),
                                                separator,
                                            });
                                        }
                                        "map" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayMap {
                                                    array: Box::new(array_expr),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "filter" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayFilter {
                                                    array: Box::new(array_expr),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "forEach" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayForEach {
                                                    array: Box::new(array_expr),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "find" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayFind {
                                                    array: Box::new(array_expr),
                                                    callback: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "indexOf" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayIndexOf {
                                                    array: Box::new(array_expr),
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "includes" => {
                                            if args.len() >= 1 {
                                                return Ok(Expr::ArrayIncludes {
                                                    array: Box::new(array_expr),
                                                    value: Box::new(args.into_iter().next().unwrap()),
                                                });
                                            }
                                        }
                                        "slice" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let start = args_iter.next().unwrap();
                                                let end = args_iter.next();
                                                return Ok(Expr::ArraySlice {
                                                    array: Box::new(array_expr),
                                                    start: Box::new(start),
                                                    end: end.map(Box::new),
                                                });
                                            }
                                        }
                                        "reduce" => {
                                            if args.len() >= 1 {
                                                let mut args_iter = args.into_iter();
                                                let callback = args_iter.next().unwrap();
                                                let initial = args_iter.next().map(Box::new);
                                                return Ok(Expr::ArrayReduce {
                                                    array: Box::new(array_expr),
                                                    callback: Box::new(callback),
                                                    initial,
                                                });
                                            }
                                        }
                                        _ => {} // Fall through for other methods
                                    }
                                }
                            }
                        }
                    }

                    // Check for global built-in function calls (parseInt, parseFloat, Number, String, isNaN, isFinite)
                    if let ast::Expr::Ident(ident) = expr.as_ref() {
                        let func_name = ident.sym.as_ref();
                        match func_name {
                            "parseInt" => {
                                let string_arg = if args.len() >= 1 {
                                    Box::new(args.remove(0))
                                } else {
                                    return Err(anyhow!("parseInt requires at least one argument"));
                                };
                                let radix_arg = if !args.is_empty() {
                                    Some(Box::new(args.remove(0)))
                                } else {
                                    None
                                };
                                return Ok(Expr::ParseInt { string: string_arg, radix: radix_arg });
                            }
                            "parseFloat" => {
                                if args.len() >= 1 {
                                    return Ok(Expr::ParseFloat(Box::new(args.remove(0))));
                                } else {
                                    return Err(anyhow!("parseFloat requires one argument"));
                                }
                            }
                            "Number" => {
                                if args.len() >= 1 {
                                    return Ok(Expr::NumberCoerce(Box::new(args.remove(0))));
                                } else {
                                    // Number() with no args returns 0
                                    return Ok(Expr::Number(0.0));
                                }
                            }
                            "String" => {
                                if args.len() >= 1 {
                                    return Ok(Expr::StringCoerce(Box::new(args.remove(0))));
                                } else {
                                    // String() with no args returns ""
                                    return Ok(Expr::String(String::new()));
                                }
                            }
                            "isNaN" => {
                                if args.len() >= 1 {
                                    return Ok(Expr::IsNaN(Box::new(args.remove(0))));
                                } else {
                                    return Err(anyhow!("isNaN requires one argument"));
                                }
                            }
                            "isFinite" => {
                                if args.len() >= 1 {
                                    return Ok(Expr::IsFinite(Box::new(args.remove(0))));
                                } else {
                                    return Err(anyhow!("isFinite requires one argument"));
                                }
                            }
                            "fetch" => {
                                // Handle fetch(url) and fetch(url, options)
                                // Extract URL (first argument)
                                let url = if args.len() >= 1 {
                                    args.remove(0)
                                } else {
                                    return Err(anyhow!("fetch requires at least a URL argument"));
                                };

                                // Check if there's an options object (second argument)
                                if args.len() >= 1 {
                                    // Extract options from the object literal
                                    // We need to get the original AST to extract the object properties
                                    if let Some(options_arg) = call.args.get(1) {
                                        if let ast::Expr::Object(obj) = &*options_arg.expr {
                                            // Extract method, body, and headers from options
                                            let mut method = Expr::String("GET".to_string());
                                            let mut body = Expr::Undefined;
                                            let mut headers_obj: Vec<(String, Expr)> = Vec::new();

                                            for prop in &obj.props {
                                                if let ast::PropOrSpread::Prop(prop) = prop {
                                                    if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                                                        let key = match &kv.key {
                                                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                                                            ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                                                            _ => continue,
                                                        };
                                                        match key.as_str() {
                                                            "method" => {
                                                                method = lower_expr(ctx, &kv.value)?;
                                                            }
                                                            "body" => {
                                                                body = lower_expr(ctx, &kv.value)?;
                                                            }
                                                            "headers" => {
                                                                // Extract headers object
                                                                if let ast::Expr::Object(headers_ast) = &*kv.value {
                                                                    for hprop in &headers_ast.props {
                                                                        if let ast::PropOrSpread::Prop(hprop) = hprop {
                                                                            if let ast::Prop::KeyValue(hkv) = hprop.as_ref() {
                                                                                let hkey = match &hkv.key {
                                                                                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                                                                                    ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                                                                                    _ => continue,
                                                                                };
                                                                                let hval = lower_expr(ctx, &hkv.value)?;
                                                                                headers_obj.push((hkey, hval));
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            }

                                            // Create a FetchWithOptions expression
                                            return Ok(Expr::FetchWithOptions {
                                                url: Box::new(url),
                                                method: Box::new(method),
                                                body: Box::new(body),
                                                headers: headers_obj,
                                            });
                                        }
                                    }
                                }

                                // Simple fetch(url) with no options - use GET
                                return Ok(Expr::FetchWithOptions {
                                    url: Box::new(url),
                                    method: Box::new(Expr::String("GET".to_string())),
                                    body: Box::new(Expr::Undefined),
                                    headers: Vec::new(),
                                });
                            }
                            _ => {} // Fall through to generic handling
                        }

                        // Check if this is a named import from child_process (e.g., execSync, spawnSync)
                        if let Some((module_name, _method)) = ctx.lookup_native_module(func_name) {
                            if module_name == "child_process" {
                                match func_name {
                                    "execSync" => {
                                        if args.len() >= 1 {
                                            let mut args_iter = args.into_iter();
                                            let command = args_iter.next().unwrap();
                                            let options = args_iter.next().map(Box::new);
                                            return Ok(Expr::ChildProcessExecSync {
                                                command: Box::new(command),
                                                options,
                                            });
                                        }
                                    }
                                    "spawnSync" => {
                                        if args.len() >= 1 {
                                            let mut args_iter = args.into_iter();
                                            let command = args_iter.next().unwrap();
                                            let spawn_args = args_iter.next().map(Box::new);
                                            let options = args_iter.next().map(Box::new);
                                            return Ok(Expr::ChildProcessSpawnSync {
                                                command: Box::new(command),
                                                args: spawn_args,
                                                options,
                                            });
                                        }
                                    }
                                    "spawn" => {
                                        if args.len() >= 1 {
                                            let mut args_iter = args.into_iter();
                                            let command = args_iter.next().unwrap();
                                            let spawn_args = args_iter.next().map(Box::new);
                                            let options = args_iter.next().map(Box::new);
                                            return Ok(Expr::ChildProcessSpawn {
                                                command: Box::new(command),
                                                args: spawn_args,
                                                options,
                                            });
                                        }
                                    }
                                    "exec" => {
                                        if args.len() >= 1 {
                                            let mut args_iter = args.into_iter();
                                            let command = args_iter.next().unwrap();
                                            let options = args_iter.next().map(Box::new);
                                            let callback = args_iter.next().map(Box::new);
                                            return Ok(Expr::ChildProcessExec {
                                                command: Box::new(command),
                                                options,
                                                callback,
                                            });
                                        }
                                    }
                                    _ => {} // Fall through
                                }
                            }

                            // Check if this is a named import from path (e.g., join, dirname, basename)
                            if module_name == "path" {
                                match func_name {
                                    "join" => {
                                        if args.len() >= 2 {
                                            let mut iter = args.into_iter();
                                            let a = iter.next().unwrap();
                                            let b = iter.next().unwrap();
                                            return Ok(Expr::PathJoin(Box::new(a), Box::new(b)));
                                        }
                                    }
                                    "dirname" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::PathDirname(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "basename" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::PathBasename(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "extname" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::PathExtname(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "resolve" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::PathResolve(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    _ => {} // Fall through
                                }
                            }

                            // Check if this is a named import from fs (e.g., existsSync, mkdirSync, etc.)
                            if module_name == "fs" {
                                match func_name {
                                    "readFileSync" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::FsReadFileSync(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "writeFileSync" => {
                                        if args.len() >= 2 {
                                            let mut iter = args.into_iter();
                                            let path = iter.next().unwrap();
                                            let content = iter.next().unwrap();
                                            return Ok(Expr::FsWriteFileSync(Box::new(path), Box::new(content)));
                                        }
                                    }
                                    "existsSync" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::FsExistsSync(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "mkdirSync" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::FsMkdirSync(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "unlinkSync" => {
                                        if args.len() >= 1 {
                                            return Ok(Expr::FsUnlinkSync(Box::new(args.into_iter().next().unwrap())));
                                        }
                                    }
                                    "appendFileSync" => {
                                        if args.len() >= 2 {
                                            let mut iter = args.into_iter();
                                            let path = iter.next().unwrap();
                                            let content = iter.next().unwrap();
                                            return Ok(Expr::FsAppendFileSync(Box::new(path), Box::new(content)));
                                        }
                                    }
                                    _ => {} // Fall through
                                }
                            }
                        }

                        // Check if this is a direct call on an aliased named import
                        // e.g., uuid() where import { v4 as uuid } from 'uuid'
                        if let Some((module_name, Some(method_name))) = ctx.lookup_native_module(func_name) {
                            return Ok(Expr::NativeMethodCall {
                                module: module_name.to_string(),
                                class_name: None,
                                object: None,
                                method: method_name.to_string(),
                                args,
                            });
                        }

                        // Check if this is a call on a default import from a native module
                        // e.g., Fastify() where import Fastify from 'fastify'
                        if let Some((module_name, None)) = ctx.lookup_native_module(func_name) {
                            return Ok(Expr::NativeMethodCall {
                                module: module_name.to_string(),
                                class_name: None,
                                object: None,
                                method: "default".to_string(), // Use "default" for default export calls
                                args,
                            });
                        }
                    }

                    let callee_expr = lower_expr(ctx, expr)?;

                    // Fill in default arguments if callee is a known function
                    let mut args = args;
                    if let Expr::FuncRef(func_id) = &callee_expr {
                        if let Some((defaults, param_ids)) = ctx.lookup_func_defaults(*func_id) {
                            let defaults = defaults.to_vec();
                            let param_ids = param_ids.to_vec();
                            let num_provided = args.len();
                            // Build substitution map: callee param LocalId -> actual arg expression
                            // For provided args, map to the caller's arg expression
                            // For defaulted args, map to the expanded default (built incrementally)
                            let mut param_map: Vec<(LocalId, Expr)> = Vec::new();
                            for i in 0..param_ids.len().min(num_provided) {
                                param_map.push((param_ids[i], args[i].clone()));
                            }
                            // Fill in missing arguments with their defaults, substituting
                            // any parameter references to use the caller's scope
                            for i in num_provided..defaults.len() {
                                if let Some(default_expr) = &defaults[i] {
                                    let substituted = LoweringContext::substitute_param_refs_in_default(
                                        default_expr, &param_map
                                    );
                                    // Add this expanded default to the map so later defaults
                                    // can reference it (e.g., c = b where b was also defaulted)
                                    if i < param_ids.len() {
                                        param_map.push((param_ids[i], substituted.clone()));
                                    }
                                    args.push(substituted);
                                }
                            }
                        }
                    }

                    let callee = Box::new(callee_expr);
                    // Extract explicit type arguments if present (e.g., identity<number>(x))
                    let type_args = call.type_args.as_ref()
                        .map(|ta| ta.params.iter()
                            .map(|t| extract_ts_type_with_ctx(t, Some(ctx)))
                            .collect())
                        .unwrap_or_default();

                    // Use CallSpread if any argument has spread
                    if let Some(spread_args) = spread_args {
                        Ok(Expr::CallSpread { callee, args: spread_args, type_args })
                    } else {
                        Ok(Expr::Call { callee, args, type_args })
                    }
                }
                ast::Callee::Import(_) => {
                    // Dynamic import: import('module')
                    // Extract the module path from the first argument if available
                    let module_path = if let Some(first_arg) = args.first() {
                        if let Expr::String(s) = first_arg {
                            s.clone()
                        } else {
                            "<dynamic>".to_string()
                        }
                    } else {
                        "<unknown>".to_string()
                    };
                    eprintln!("Warning: Dynamic import('{}') not fully supported, returning undefined", module_path);
                    // Dynamic imports return a Promise that resolves to the module
                    // For now, return undefined as we'd need full runtime support
                    Ok(Expr::Undefined)
                }
            }
        }
        ast::Expr::Member(member) => {
            // Check if this is process.argv access
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                if obj_ident.sym.as_ref() == "process" {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        if prop_ident.sym.as_ref() == "argv" {
                            return Ok(Expr::ProcessArgv);
                        }
                    }
                }
            }

            // Check if this is a process.env.VARNAME or process.env[expr] access
            if let ast::Expr::Member(inner_member) = member.obj.as_ref() {
                if let ast::Expr::Ident(obj_ident) = inner_member.obj.as_ref() {
                    if obj_ident.sym.as_ref() == "process" {
                        if let ast::MemberProp::Ident(prop_ident) = &inner_member.prop {
                            if prop_ident.sym.as_ref() == "env" {
                                // This is process.env access
                                match &member.prop {
                                    ast::MemberProp::Ident(var_ident) => {
                                        // process.env.VARNAME (static key)
                                        let var_name = var_ident.sym.to_string();
                                        return Ok(Expr::EnvGet(var_name));
                                    }
                                    ast::MemberProp::Computed(computed) => {
                                        // process.env[expr] (dynamic key)
                                        let key_expr = Box::new(lower_expr(ctx, &computed.expr)?);
                                        return Ok(Expr::EnvGetDynamic(key_expr));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // Check if this is an enum member access (e.g., Color.Red)
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.to_string();
                if ctx.lookup_enum(&obj_name).is_some() {
                    // This is an enum access
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        let member_name = prop_ident.sym.to_string();
                        return Ok(Expr::EnumMember {
                            enum_name: obj_name,
                            member_name,
                        });
                    }
                }
            }

            // Check if this is a static field access (e.g., Counter.count)
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.to_string();
                if ctx.lookup_class(&obj_name).is_some() {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        let field_name = prop_ident.sym.to_string();
                        if ctx.has_static_field(&obj_name, &field_name) {
                            return Ok(Expr::StaticFieldGet {
                                class_name: obj_name,
                                field_name,
                            });
                        }
                    }
                }
            }

            // Check if this is os.EOL property access
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.as_ref();
                let is_os_module = obj_name == "os" ||
                    ctx.lookup_builtin_module_alias(obj_name) == Some("os");
                if is_os_module {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        if prop_ident.sym.as_ref() == "EOL" {
                            return Ok(Expr::OsEOL);
                        }
                    }
                }
            }

            // Check for native instance property access (e.g., response.status, response.ok)
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.to_string();
                // Clone module_name early to avoid borrow issues
                let native_instance = ctx.lookup_native_instance(&obj_name)
                    .map(|(m, _c)| m.to_string());
                if let Some(module_name) = native_instance {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        let property_name = prop_ident.sym.to_string();
                        // For properties that map to FFI functions, generate a NativeMethodCall
                        // with no args (property getter)
                        let object_expr = lower_expr(ctx, &member.obj)?;
                        return Ok(Expr::NativeMethodCall {
                            module: module_name,
                            class_name: None,
                            object: Some(Box::new(object_expr)),
                            method: property_name,
                            args: Vec::new(),
                        });
                    }
                }
            }

            let object = Box::new(lower_expr(ctx, &member.obj)?);
            match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let property = ident.sym.to_string();
                    Ok(Expr::PropertyGet { object, property })
                }
                ast::MemberProp::Computed(computed) => {
                    let index = Box::new(lower_expr(ctx, &computed.expr)?);
                    Ok(Expr::IndexGet { object, index })
                }
                ast::MemberProp::PrivateName(private) => {
                    // Private field access: this.#field -> PropertyGet with "#field"
                    let property = format!("#{}", private.name.to_string());
                    Ok(Expr::PropertyGet { object, property })
                }
            }
        }
        ast::Expr::Paren(paren) => {
            lower_expr(ctx, &paren.expr)
        }
        ast::Expr::Assign(assign) => {
            let rhs = lower_expr(ctx, &assign.right)?;

            // Handle compound assignment operators (+=, -=, *=, /=, etc.)
            let value = match assign.op {
                ast::AssignOp::Assign => Box::new(rhs),
                ast::AssignOp::AddAssign => {
                    // a += b becomes a = a + b
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Add,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::SubAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Sub,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::MulAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Mul,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::DivAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Div,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::ModAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Mod,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::BitAndAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::BitAnd,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::BitOrAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::BitOr,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::BitXorAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::BitXor,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::LShiftAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Shl,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::RShiftAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Shr,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::ZeroFillRShiftAssign => {
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::UShr,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::ExpAssign => {
                    // a **= b becomes a = a ** b
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Binary {
                        op: BinaryOp::Pow,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::AndAssign => {
                    // a &&= b becomes a = a && b (short-circuit: only evaluates b if a is truthy)
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Logical {
                        op: LogicalOp::And,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::OrAssign => {
                    // a ||= b becomes a = a || b (short-circuit: only evaluates b if a is falsy)
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Logical {
                        op: LogicalOp::Or,
                        left,
                        right: Box::new(rhs),
                    })
                }
                ast::AssignOp::NullishAssign => {
                    // a ??= b becomes a = a ?? b (short-circuit: only evaluates b if a is null/undefined)
                    let left = Box::new(lower_assign_target_to_expr(ctx, &assign.left)?);
                    Box::new(Expr::Logical {
                        op: LogicalOp::Coalesce,
                        left,
                        right: Box::new(rhs),
                    })
                }
                _ => return Err(anyhow!("Unsupported assignment operator: {:?}", assign.op)),
            };

            match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
                    let name = ident.id.sym.to_string();
                    if let Some(id) = ctx.lookup_local(&name) {
                        Ok(Expr::LocalSet(id, value))
                    } else {
                        Err(anyhow!("Assignment to undeclared variable: {}", name))
                    }
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
                    // Check if this is a static field assignment (e.g., Counter.count = 5)
                    if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                        let obj_name = obj_ident.sym.to_string();
                        if ctx.lookup_class(&obj_name).is_some() {
                            if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                                let field_name = prop_ident.sym.to_string();
                                if ctx.has_static_field(&obj_name, &field_name) {
                                    return Ok(Expr::StaticFieldSet {
                                        class_name: obj_name,
                                        field_name,
                                        value,
                                    });
                                }
                            }
                        }
                    }

                    let object = Box::new(lower_expr(ctx, &member.obj)?);
                    match &member.prop {
                        ast::MemberProp::Ident(ident) => {
                            let property = ident.sym.to_string();
                            Ok(Expr::PropertySet { object, property, value })
                        }
                        ast::MemberProp::Computed(computed) => {
                            let index = Box::new(lower_expr(ctx, &computed.expr)?);
                            Ok(Expr::IndexSet { object, index, value })
                        }
                        ast::MemberProp::PrivateName(private) => {
                            // Private field assignment: this.#field = value
                            let property = format!("#{}", private.name.to_string());
                            Ok(Expr::PropertySet { object, property, value })
                        }
                    }
                }
                ast::AssignTarget::Pat(pat) => {
                    // Destructuring assignment: [a, b] = expr or { a, b } = expr
                    // We need to lower this to a sequence of assignments
                    lower_destructuring_assignment(ctx, pat, value)
                }
                other => Err(anyhow!("Unsupported assignment target: {:?}", other)),
            }
        }
        ast::Expr::Cond(cond) => {
            let condition = Box::new(lower_expr(ctx, &cond.test)?);
            let then_expr = Box::new(lower_expr(ctx, &cond.cons)?);
            let else_expr = Box::new(lower_expr(ctx, &cond.alt)?);
            Ok(Expr::Conditional { condition, then_expr, else_expr })
        }
        ast::Expr::Array(array) => {
            // Check if any elements are spread elements
            let has_spread = array.elems.iter()
                .filter_map(|elem| elem.as_ref())
                .any(|elem| elem.spread.is_some());

            if has_spread {
                // Use ArraySpread for arrays with spread elements
                let elements = array.elems.iter()
                    .filter_map(|elem| elem.as_ref())
                    .map(|elem| {
                        let expr = lower_expr(ctx, &elem.expr)?;
                        if elem.spread.is_some() {
                            Ok(ArrayElement::Spread(expr))
                        } else {
                            Ok(ArrayElement::Expr(expr))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::ArraySpread(elements))
            } else {
                // No spread elements, use regular Array
                let elements = array.elems.iter()
                    .filter_map(|elem| elem.as_ref())
                    .map(|elem| lower_expr(ctx, &elem.expr))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Array(elements))
            }
        }
        ast::Expr::Object(obj) => {
            let props = obj.props.iter()
                .filter_map(|prop| {
                    match prop {
                        ast::PropOrSpread::Prop(prop) => {
                            match prop.as_ref() {
                                ast::Prop::KeyValue(kv) => {
                                    let key = match &kv.key {
                                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                                        ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                                        _ => return None,
                                    };
                                    let value = lower_expr(ctx, &kv.value).ok()?;
                                    Some(Ok((key, value)))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Expr::Object(props))
        }
        ast::Expr::This(_) => {
            // Always use Expr::This - the codegen will handle it with ThisContext
            Ok(Expr::This)
        }
        ast::Expr::New(new_expr) => {
            // Try to extract class name from callee
            match new_expr.callee.as_ref() {
                ast::Expr::Ident(ident) => {
                    let class_name = ident.sym.to_string();

                    // Handle built-in types
                    if class_name == "Map" {
                        // new Map() -> create empty map
                        return Ok(Expr::MapNew);
                    }
                    if class_name == "Set" {
                        // new Set() -> create empty set
                        return Ok(Expr::SetNew);
                    }
                    if class_name == "Date" {
                        // new Date() or new Date(timestamp)
                        let args = new_expr.args.as_ref()
                            .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                            .transpose()?
                            .unwrap_or_default();
                        if args.is_empty() {
                            return Ok(Expr::DateNew(None));
                        } else {
                            return Ok(Expr::DateNew(Some(Box::new(args.into_iter().next().unwrap()))));
                        }
                    }
                    // Handle Error and its subclasses
                    if class_name == "Error" || class_name == "TypeError" || class_name == "RangeError"
                        || class_name == "ReferenceError" || class_name == "SyntaxError"
                        || class_name == "BugIndicatingError" {
                        // new Error() or new Error(message)
                        let args = new_expr.args.as_ref()
                            .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                            .transpose()?
                            .unwrap_or_default();
                        if args.is_empty() {
                            return Ok(Expr::ErrorNew(None));
                        } else {
                            return Ok(Expr::ErrorNew(Some(Box::new(args.into_iter().next().unwrap()))));
                        }
                    }

                    // Handle URL class
                    if class_name == "URL" {
                        // new URL(url) or new URL(url, base)
                        let args = new_expr.args.as_ref()
                            .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                            .transpose()?
                            .unwrap_or_default();
                        let mut args_iter = args.into_iter();
                        let url_arg = args_iter.next()
                            .ok_or_else(|| anyhow!("URL constructor requires at least 1 argument"))?;
                        let base_arg = args_iter.next();
                        return Ok(Expr::UrlNew {
                            url: Box::new(url_arg),
                            base: base_arg.map(Box::new),
                        });
                    }

                    // Handle URLSearchParams class
                    if class_name == "URLSearchParams" {
                        // new URLSearchParams() or new URLSearchParams(init)
                        let args = new_expr.args.as_ref()
                            .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                            .transpose()?
                            .unwrap_or_default();
                        let init_arg = args.into_iter().next();
                        return Ok(Expr::UrlSearchParamsNew(init_arg.map(Box::new)));
                    }

                    // Handle Uint8Array constructor
                    if class_name == "Uint8Array" {
                        // new Uint8Array() or new Uint8Array(length) or new Uint8Array(array)
                        let args = new_expr.args.as_ref()
                            .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                            .transpose()?
                            .unwrap_or_default();
                        if args.is_empty() {
                            return Ok(Expr::Uint8ArrayNew(None));
                        } else {
                            return Ok(Expr::Uint8ArrayNew(Some(Box::new(args.into_iter().next().unwrap()))));
                        }
                    }

                    let args = new_expr.args.as_ref()
                        .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                        .transpose()?
                        .unwrap_or_default();
                    // Extract explicit type arguments if present (e.g., new Box<number>(42))
                    let type_args = new_expr.type_args.as_ref()
                        .map(|ta| ta.params.iter()
                            .map(|t| extract_ts_type_with_ctx(t, Some(ctx)))
                            .collect())
                        .unwrap_or_default();
                    Ok(Expr::New { class_name, args, type_args })
                }
                // Non-identifier callee (e.g., new (condition ? A : B)() or new someVar())
                _ => {
                    let callee = Box::new(lower_expr(ctx, &new_expr.callee)?);
                    let args = new_expr.args.as_ref()
                        .map(|args| args.iter().map(|a| lower_expr(ctx, &a.expr)).collect::<Result<Vec<_>>>())
                        .transpose()?
                        .unwrap_or_default();
                    Ok(Expr::NewDynamic { callee, args })
                }
            }
        }
        ast::Expr::Arrow(arrow) => {
            // Lower arrow function to a closure
            let func_id = ctx.fresh_func();
            let scope_mark = ctx.enter_scope();

            // Track which locals exist before entering the closure scope
            let outer_locals: Vec<(String, LocalId)> = ctx.locals.iter()
                .map(|(name, id, _)| (name.clone(), *id))
                .collect();

            // Lower parameters and collect destructuring info
            let mut params = Vec::new();
            let mut destructuring_params: Vec<(LocalId, ast::Pat)> = Vec::new();
            for param in &arrow.params {
                let param_name = get_pat_name(param)?;
                let param_default = get_param_default(ctx, param)?;
                let is_rest = is_rest_param(param);
                let param_ty = get_pat_type(param, ctx);
                let param_id = ctx.define_local(param_name.clone(), param_ty.clone());
                params.push(Param {
                    id: param_id,
                    name: param_name,
                    ty: param_ty,
                    default: param_default,
                    is_rest,
                });
                // Track destructuring patterns to generate extraction statements
                if is_destructuring_pattern(param) {
                    destructuring_params.push((param_id, param.clone()));
                }
            }

            // Generate Let statements for destructuring patterns BEFORE lowering body
            // This ensures the destructured variable names are defined when the body references them
            let mut destructuring_stmts = Vec::new();
            for (param_id, pat) in &destructuring_params {
                let stmts = generate_param_destructuring_stmts(ctx, pat, *param_id)?;
                destructuring_stmts.extend(stmts);
            }

            // Lower body
            let mut body = match &*arrow.body {
                ast::BlockStmtOrExpr::BlockStmt(block) => lower_block_stmt(ctx, block)?,
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let return_expr = lower_expr(ctx, expr)?;
                    vec![Stmt::Return(Some(return_expr))]
                }
            };

            // Prepend destructuring statements to body
            if !destructuring_stmts.is_empty() {
                let mut new_body = destructuring_stmts;
                new_body.append(&mut body);
                body = new_body;
            }

            ctx.exit_scope(scope_mark);

            // Detect captured variables: locals referenced in the body that were defined in outer scope
            let mut all_refs = Vec::new();
            for stmt in &body {
                collect_local_refs_stmt(stmt, &mut all_refs);
            }

            // Filter to only include outer locals (not parameters or locals defined within the closure)
            let outer_local_ids: std::collections::HashSet<LocalId> = outer_locals.iter()
                .map(|(_, id)| *id)
                .collect();
            let param_ids: std::collections::HashSet<LocalId> = params.iter()
                .map(|p| p.id)
                .collect();

            // Find unique captures: refs that are in outer_locals but not params
            let mut captures: Vec<LocalId> = all_refs.into_iter()
                .filter(|id| outer_local_ids.contains(id) && !param_ids.contains(id))
                .collect();
            captures.sort();
            captures.dedup();

            // Detect which captures are assigned to inside the closure (need boxing)
            let mut all_assigned = Vec::new();
            for stmt in &body {
                collect_assigned_locals_stmt(stmt, &mut all_assigned);
            }
            let assigned_set: std::collections::HashSet<LocalId> = all_assigned.into_iter().collect();
            let mutable_captures: Vec<LocalId> = captures.iter()
                .filter(|id| assigned_set.contains(id))
                .copied()
                .collect();

            // Check if this arrow function uses `this` (needs to capture it from enclosing scope)
            let captures_this = closure_uses_this(&body);

            // Store enclosing class name for arrow functions that capture `this`
            let enclosing_class = if captures_this {
                ctx.current_class.clone()
            } else {
                None
            };

            Ok(Expr::Closure {
                func_id,
                params,
                return_type: Type::Any,
                body,
                captures,
                mutable_captures,
                captures_this,
                enclosing_class,
                is_async: arrow.is_async,
            })
        }
        ast::Expr::Fn(fn_expr) => {
            // Lower function expression to a closure (similar to arrow)
            let func_id = ctx.fresh_func();
            let scope_mark = ctx.enter_scope();

            // Track which locals exist before entering the closure scope
            let outer_locals: Vec<(String, LocalId)> = ctx.locals.iter()
                .map(|(name, id, _)| (name.clone(), *id))
                .collect();

            // Lower parameters and collect destructuring info
            let mut params = Vec::new();
            let mut destructuring_params: Vec<(LocalId, ast::Pat)> = Vec::new();
            for param in &fn_expr.function.params {
                let param_name = get_pat_name(&param.pat)?;
                let param_default = get_param_default(ctx, &param.pat)?;
                let is_rest = is_rest_param(&param.pat);
                let param_id = ctx.define_local(param_name.clone(), Type::Any);
                params.push(Param {
                    id: param_id,
                    name: param_name,
                    ty: Type::Any,
                    default: param_default,
                    is_rest,
                });
                // Track destructuring patterns to generate extraction statements
                if is_destructuring_pattern(&param.pat) {
                    destructuring_params.push((param_id, param.pat.clone()));
                }
            }

            // Generate Let statements for destructuring patterns BEFORE lowering body
            let mut destructuring_stmts = Vec::new();
            for (param_id, pat) in &destructuring_params {
                let stmts = generate_param_destructuring_stmts(ctx, pat, *param_id)?;
                destructuring_stmts.extend(stmts);
            }

            // Lower body
            let mut body = if let Some(ref block) = fn_expr.function.body {
                lower_block_stmt(ctx, block)?
            } else {
                Vec::new()
            };

            // Prepend destructuring statements to body
            if !destructuring_stmts.is_empty() {
                let mut new_body = destructuring_stmts;
                new_body.append(&mut body);
                body = new_body;
            }

            ctx.exit_scope(scope_mark);

            // Detect captured variables
            let mut all_refs = Vec::new();
            for stmt in &body {
                collect_local_refs_stmt(stmt, &mut all_refs);
            }

            let outer_local_ids: std::collections::HashSet<LocalId> = outer_locals.iter()
                .map(|(_, id)| *id)
                .collect();
            let param_ids: std::collections::HashSet<LocalId> = params.iter()
                .map(|p| p.id)
                .collect();

            let mut captures: Vec<LocalId> = all_refs.into_iter()
                .filter(|id| outer_local_ids.contains(id) && !param_ids.contains(id))
                .collect();
            captures.sort();
            captures.dedup();

            // Detect which captures are assigned to inside the closure (need boxing)
            let mut all_assigned = Vec::new();
            for stmt in &body {
                collect_assigned_locals_stmt(stmt, &mut all_assigned);
            }
            let assigned_set: std::collections::HashSet<LocalId> = all_assigned.into_iter().collect();
            let mutable_captures: Vec<LocalId> = captures.iter()
                .filter(|id| assigned_set.contains(id))
                .copied()
                .collect();

            // Regular function expressions do NOT capture `this` from enclosing scope
            // (they have their own `this` binding determined by how they're called)
            let captures_this = false;

            Ok(Expr::Closure {
                func_id,
                params,
                return_type: Type::Any,
                body,
                captures,
                mutable_captures,
                captures_this,
                enclosing_class: None,
                is_async: fn_expr.function.is_async,
            })
        }
        ast::Expr::Await(await_expr) => {
            let inner = Box::new(lower_expr(ctx, &await_expr.arg)?);
            Ok(Expr::Await(inner))
        }
        ast::Expr::SuperProp(super_prop) => {
            // super.property access - used in super.method() calls
            // When used as a call target, the Call expression will detect this
            // For now, we'll error on direct property access (super.prop without call)
            match &super_prop.prop {
                ast::SuperProp::Ident(_ident) => {
                    // This is typically used in Call expressions like super.method()
                    // We return a placeholder that will be handled specially
                    Err(anyhow!("Direct super property access not yet supported, use super.method()"))
                }
                ast::SuperProp::Computed(_) => {
                    Err(anyhow!("Computed super property access not supported"))
                }
            }
        }
        ast::Expr::Update(update) => {
            // Handle ++x, x++, --x, x--
            let binary_op = match update.op {
                ast::UpdateOp::PlusPlus => BinaryOp::Add,
                ast::UpdateOp::MinusMinus => BinaryOp::Sub,
            };

            match update.arg.as_ref() {
                // Simple identifier: x++ or ++x
                ast::Expr::Ident(ident) => {
                    let name = ident.sym.to_string();
                    let id = ctx.lookup_local(&name)
                        .ok_or_else(|| anyhow!("Undefined variable in update expression: {}", name))?;
                    let op = match update.op {
                        ast::UpdateOp::PlusPlus => UpdateOp::Increment,
                        ast::UpdateOp::MinusMinus => UpdateOp::Decrement,
                    };
                    Ok(Expr::Update {
                        id,
                        op,
                        prefix: update.prefix,
                    })
                }
                // Member expression: this.count++ or obj.prop++ or obj[key]++
                ast::Expr::Member(member) => {
                    let object = lower_expr(ctx, &member.obj)?;
                    match &member.prop {
                        ast::MemberProp::Ident(ident) => {
                            let property = ident.sym.to_string();
                            // Desugar: this.count++ becomes (tmp = this.count, this.count = tmp + 1, tmp)
                            // For prefix ++this.count becomes (this.count = this.count + 1, this.count)
                            // We simplify to just: this.count = this.count + 1
                            // The return value semantics are handled at codegen
                            Ok(Expr::PropertyUpdate {
                                object: Box::new(object),
                                property,
                                op: binary_op,
                                prefix: update.prefix,
                            })
                        }
                        ast::MemberProp::PrivateName(priv_name) => {
                            let property = format!("#{}", priv_name.name);
                            Ok(Expr::PropertyUpdate {
                                object: Box::new(object),
                                property,
                                op: binary_op,
                                prefix: update.prefix,
                            })
                        }
                        ast::MemberProp::Computed(comp) => {
                            // Computed property: obj[key]++
                            let index = lower_expr(ctx, &comp.expr)?;
                            Ok(Expr::IndexUpdate {
                                object: Box::new(object),
                                index: Box::new(index),
                                op: binary_op,
                                prefix: update.prefix,
                            })
                        }
                    }
                }
                _ => Err(anyhow!("Update expression only supports identifiers and member expressions")),
            }
        }
        ast::Expr::Tpl(tpl) => {
            // Template literal: `Hello, ${name}!`
            // quasis = ["Hello, ", "!"], exprs = [name]
            // We desugar this to string concatenation

            if tpl.quasis.is_empty() {
                return Ok(Expr::String(String::new()));
            }

            // Start with the first quasi
            let first_raw = tpl.quasis.first()
                .map(|q| q.raw.as_ref())
                .unwrap_or("");
            let mut result = Expr::String(unescape_template(first_raw));

            // Interleave expressions and remaining quasis
            for (i, expr) in tpl.exprs.iter().enumerate() {
                let lowered = lower_expr(ctx, expr)?;
                // Concatenate: result + toString(expr)
                result = Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(result),
                    right: Box::new(lowered),
                };

                // Add the next quasi (if it's non-empty)
                if let Some(quasi) = tpl.quasis.get(i + 1) {
                    let quasi_str: &str = quasi.raw.as_ref();
                    if !quasi_str.is_empty() {
                        result = Expr::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(result),
                            right: Box::new(Expr::String(unescape_template(quasi_str))),
                        };
                    }
                }
            }

            Ok(result)
        }
        ast::Expr::OptChain(opt_chain) => {
            // Optional chaining: obj?.prop or obj?.[index] or obj?.method()
            // Convert to: obj == null ? undefined : obj.prop
            match &*opt_chain.base {
                ast::OptChainBase::Member(member) => {
                    // obj?.prop -> obj == null ? undefined : obj.prop
                    let obj_expr = lower_expr(ctx, &member.obj)?;

                    // Get the property access
                    let prop_expr = match &member.prop {
                        ast::MemberProp::Ident(ident) => {
                            let prop_name = ident.sym.to_string();
                            Expr::PropertyGet {
                                object: Box::new(obj_expr.clone()),
                                property: prop_name,
                            }
                        }
                        ast::MemberProp::Computed(comp) => {
                            let index = lower_expr(ctx, &comp.expr)?;
                            Expr::IndexGet {
                                object: Box::new(obj_expr.clone()),
                                index: Box::new(index),
                            }
                        }
                        _ => return Err(anyhow!("Unsupported optional chain property type")),
                    };

                    // Generate: obj == null ? undefined : prop_expr
                    Ok(Expr::Conditional {
                        condition: Box::new(Expr::Compare {
                            op: CompareOp::Eq,
                            left: Box::new(obj_expr),
                            right: Box::new(Expr::Null),
                        }),
                        then_expr: Box::new(Expr::Undefined),
                        else_expr: Box::new(prop_expr),
                    })
                }
                ast::OptChainBase::Call(call) => {
                    // obj?.method() -> obj == null ? undefined : obj.method()
                    let callee = &call.callee;

                    // Check for spread arguments
                    let has_spread = call.args.iter().any(|arg| arg.spread.is_some());

                    let args = call.args.iter()
                        .map(|arg| lower_expr(ctx, &arg.expr))
                        .collect::<Result<Vec<_>>>()?;

                    let callee_expr = lower_expr(ctx, callee)?;

                    // For method calls, we need to check the object
                    // This is simplified - full implementation would need more context
                    if has_spread {
                        let spread_args: Vec<CallArg> = call.args.iter().zip(args.iter())
                            .map(|(ast_arg, lowered)| {
                                if ast_arg.spread.is_some() {
                                    CallArg::Spread(lowered.clone())
                                } else {
                                    CallArg::Expr(lowered.clone())
                                }
                            })
                            .collect();
                        Ok(Expr::CallSpread {
                            callee: Box::new(callee_expr),
                            args: spread_args,
                            type_args: Vec::new(),
                        })
                    } else {
                        Ok(Expr::Call {
                            callee: Box::new(callee_expr),
                            args,
                            type_args: Vec::new(),
                        })
                    }
                }
            }
        }
        ast::Expr::TsAs(ts_as) => {
            // TypeScript 'as' type assertion - at runtime, just evaluate the expression
            // The type assertion is compile-time only
            lower_expr(ctx, &ts_as.expr)
        }
        ast::Expr::TsNonNull(ts_non_null) => {
            // TypeScript non-null assertion (value!) - at runtime, just the expression
            lower_expr(ctx, &ts_non_null.expr)
        }
        ast::Expr::TsTypeAssertion(ts_assertion) => {
            // TypeScript angle-bracket type assertion (<Type>value) - same as 'as', compile-time only
            lower_expr(ctx, &ts_assertion.expr)
        }
        ast::Expr::TsConstAssertion(ts_const) => {
            // TypeScript 'as const' assertion - at runtime, just evaluate the expression
            // The const assertion only affects type inference, not runtime behavior
            lower_expr(ctx, &ts_const.expr)
        }
        ast::Expr::TsSatisfies(ts_satisfies) => {
            // TypeScript 'satisfies' operator - compile-time type check only
            lower_expr(ctx, &ts_satisfies.expr)
        }
        ast::Expr::TsInstantiation(ts_inst) => {
            // TypeScript generic instantiation (func<Type>) - at runtime, just the expression
            lower_expr(ctx, &ts_inst.expr)
        }
        ast::Expr::Seq(seq) => {
            // Comma operator: evaluate all expressions left-to-right, return the last value
            // e.g., (a++, b++, c) evaluates a++, then b++, then returns c
            let mut last_expr = Expr::Undefined;
            for expr in &seq.exprs {
                last_expr = lower_expr(ctx, expr)?;
            }
            Ok(last_expr)
        }
        ast::Expr::MetaProp(meta_prop) => {
            // import.meta expression
            // We only support import.meta itself - property access (.url) is handled in Member expr
            // For now, return a placeholder object that will be handled in property access
            match meta_prop.kind {
                ast::MetaPropKind::ImportMeta => {
                    // Return the file:// URL directly for import.meta.url
                    // Since import.meta is typically accessed via .url, we generate the URL here
                    let file_url = format!("file://{}", ctx.source_file_path);
                    Ok(Expr::Object(vec![
                        ("url".to_string(), Expr::String(file_url)),
                    ]))
                }
                ast::MetaPropKind::NewTarget => {
                    // new.target - not commonly used, return undefined for now
                    Ok(Expr::Undefined)
                }
            }
        }
        _ => Err(anyhow!("Unsupported expression type: {:?}", expr)),
    }
}

/// Unescape template literal strings (handle \n, \t, etc.)
fn unescape_template(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('$') => result.push('$'),
                Some('`') => result.push('`'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

fn lower_lit(lit: &ast::Lit) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => {
            let value = n.value;
            // Check if this is an integer that fits in i64
            if value.fract() == 0.0
                && value >= i64::MIN as f64
                && value <= i64::MAX as f64
            {
                Ok(Expr::Integer(value as i64))
            } else {
                Ok(Expr::Number(value))
            }
        }
        ast::Lit::Str(s) => Ok(Expr::String(s.value.as_str().unwrap_or("").to_string())),
        ast::Lit::Bool(b) => Ok(Expr::Bool(b.value)),
        ast::Lit::Null(_) => Ok(Expr::Null),
        ast::Lit::BigInt(bi) => Ok(Expr::BigInt(bi.value.to_string())),
        ast::Lit::Regex(re) => Ok(Expr::RegExp {
            pattern: re.exp.to_string(),
            flags: re.flags.to_string(),
        }),
        _ => Err(anyhow!("Unsupported literal type")),
    }
}

/// Convert an assignment target to an expression for reading its current value
/// Used for compound assignment operators like += to read the current value before modifying
fn lower_assign_target_to_expr(ctx: &mut LoweringContext, target: &ast::AssignTarget) -> Result<Expr> {
    match target {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
            let name = ident.id.sym.to_string();
            if let Some(id) = ctx.lookup_local(&name) {
                Ok(Expr::LocalGet(id))
            } else {
                Err(anyhow!("Undefined variable in compound assignment: {}", name))
            }
        }
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
            // Check if this is a static field access
            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                let obj_name = obj_ident.sym.to_string();
                if ctx.lookup_class(&obj_name).is_some() {
                    if let ast::MemberProp::Ident(prop_ident) = &member.prop {
                        let field_name = prop_ident.sym.to_string();
                        if ctx.has_static_field(&obj_name, &field_name) {
                            return Ok(Expr::StaticFieldGet {
                                class_name: obj_name,
                                field_name,
                            });
                        }
                    }
                }
            }

            let object = Box::new(lower_expr(ctx, &member.obj)?);
            match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let property = ident.sym.to_string();
                    Ok(Expr::PropertyGet { object, property })
                }
                ast::MemberProp::Computed(computed) => {
                    let index = Box::new(lower_expr(ctx, &computed.expr)?);
                    Ok(Expr::IndexGet { object, index })
                }
                _ => Err(anyhow!("Unsupported member property in compound assignment")),
            }
        }
        _ => Err(anyhow!("Unsupported target in compound assignment")),
    }
}

fn get_binding_name(pat: &ast::Pat) -> Result<String> {
    match pat {
        ast::Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
        _ => Err(anyhow!("Unsupported binding pattern")),
    }
}

/// Static counter for generating unique synthetic names for destructuring patterns
static DESTRUCT_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn get_pat_name(pat: &ast::Pat) -> Result<String> {
    match pat {
        ast::Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
        ast::Pat::Assign(assign) => get_pat_name(&assign.left),
        ast::Pat::Rest(rest) => get_pat_name(&rest.arg),
        // For complex destructuring patterns, generate synthetic names
        // The actual destructuring will be handled at the call site or as a separate pass
        ast::Pat::Array(_) => {
            let id = DESTRUCT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(format!("__arr_destruct_{}", id))
        }
        ast::Pat::Object(_) => {
            let id = DESTRUCT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(format!("__obj_destruct_{}", id))
        }
        _ => Err(anyhow!("Unsupported pattern")),
    }
}

/// Extract the type annotation from a Pat (for arrow function parameters)
fn get_pat_type(pat: &ast::Pat, ctx: &LoweringContext) -> Type {
    match pat {
        ast::Pat::Ident(ident) => {
            ident.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Assign(assign) => get_pat_type(&assign.left, ctx),
        ast::Pat::Rest(rest) => {
            rest.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Array(arr) => {
            arr.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        ast::Pat::Object(obj) => {
            obj.type_ann.as_ref()
                .map(|ann| extract_ts_type_with_ctx(&ann.type_ann, Some(ctx)))
                .unwrap_or(Type::Any)
        }
        _ => Type::Any,
    }
}

/// Generate Let statements to extract destructured variables from a synthetic parameter.
/// For array patterns like `[a, b]`, generates:
///   let a = param[0];
///   let b = param[1];
/// For object patterns like `{a, b}`, generates:
///   let a = param.a;
///   let b = param.b;
/// Returns the statements and defines the variables in the context.
fn generate_param_destructuring_stmts(
    ctx: &mut LoweringContext,
    pat: &ast::Pat,
    param_id: LocalId,
) -> Result<Vec<Stmt>> {
    let mut stmts = Vec::new();

    match pat {
        ast::Pat::Array(arr_pat) => {
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            let id = ctx.define_local(name.clone(), Type::Any);
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                        }
                        ast::Pat::Array(nested_arr) => {
                            // Nested array destructuring: [[a, b], c]
                            // First extract the nested array element
                            let nested_id = ctx.fresh_local();
                            let nested_name = format!("__nested_{}", nested_id);
                            ctx.locals.push((nested_name.clone(), nested_id, Type::Any));
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id: nested_id,
                                name: nested_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            // Recursively generate destructuring for nested pattern
                            let nested_stmts = generate_param_destructuring_stmts(ctx, &ast::Pat::Array(nested_arr.clone()), nested_id)?;
                            stmts.extend(nested_stmts);
                        }
                        ast::Pat::Object(nested_obj) => {
                            // Nested object destructuring: [{a, b}, c]
                            let nested_id = ctx.fresh_local();
                            let nested_name = format!("__nested_{}", nested_id);
                            ctx.locals.push((nested_name.clone(), nested_id, Type::Any));
                            let index_expr = Expr::IndexGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                index: Box::new(Expr::Number(idx as f64)),
                            };
                            stmts.push(Stmt::Let {
                                id: nested_id,
                                name: nested_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            let nested_stmts = generate_param_destructuring_stmts(ctx, &ast::Pat::Object(nested_obj.clone()), nested_id)?;
                            stmts.extend(nested_stmts);
                        }
                        ast::Pat::Rest(rest_pat) => {
                            // Rest pattern: [a, ...rest]
                            // For now, skip (would need slice operation)
                            if let ast::Pat::Ident(ident) = rest_pat.arg.as_ref() {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Array(Box::new(Type::Any)));
                                // Create a slice from idx to end
                                let slice_expr = Expr::ArraySlice {
                                    array: Box::new(Expr::LocalGet(param_id)),
                                    start: Box::new(Expr::Number(idx as f64)),
                                    end: None,
                                };
                                stmts.push(Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Array(Box::new(Type::Any)),
                                    mutable: false,
                                    init: Some(slice_expr),
                                });
                            }
                        }
                        ast::Pat::Assign(assign_pat) => {
                            // Default value: [a = default, b]
                            if let ast::Pat::Ident(ident) = assign_pat.left.as_ref() {
                                let name = ident.id.sym.to_string();
                                let id = ctx.define_local(name.clone(), Type::Any);
                                let index_expr = Expr::IndexGet {
                                    object: Box::new(Expr::LocalGet(param_id)),
                                    index: Box::new(Expr::Number(idx as f64)),
                                };
                                // TODO: handle default value with nullish coalescing
                                stmts.push(Stmt::Let {
                                    id,
                                    name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(index_expr),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        ast::Pat::Object(obj_pat) => {
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ast::IdentName { sym, .. }) => sym.to_string(),
                            ast::PropName::Str(s) => String::from_utf8_lossy(s.value.as_bytes()).to_string(),
                            _ => continue,
                        };
                        if let ast::Pat::Ident(ident) = kv.value.as_ref() {
                            let name = ident.id.sym.to_string();
                            let id = ctx.define_local(name.clone(), Type::Any);
                            let prop_expr = Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(param_id)),
                                property: key,
                            };
                            stmts.push(Stmt::Let {
                                id,
                                name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(prop_expr),
                            });
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        let name = assign.key.sym.to_string();
                        let id = ctx.define_local(name.clone(), Type::Any);
                        let prop_expr = Expr::PropertyGet {
                            object: Box::new(Expr::LocalGet(param_id)),
                            property: name.clone(),
                        };
                        // TODO: handle default value with nullish coalescing
                        stmts.push(Stmt::Let {
                            id,
                            name,
                            ty: Type::Any,
                            mutable: false,
                            init: Some(prop_expr),
                        });
                    }
                    ast::ObjectPatProp::Rest(_) => {
                        // Rest pattern: {...rest} - skip for now
                    }
                }
            }
        }
        _ => {}
    }

    Ok(stmts)
}

/// Check if a pattern is a destructuring pattern (array or object)
fn is_destructuring_pattern(pat: &ast::Pat) -> bool {
    matches!(pat, ast::Pat::Array(_) | ast::Pat::Object(_))
}

/// Detect if an expression represents a native handle instance (Big, Decimal, etc.)
/// Returns the module name if it does.
fn detect_native_instance_expr(expr: &ast::Expr) -> Option<&'static str> {
    match expr {
        // new Big(...) / new Decimal(...) / new BigNumber(...)
        ast::Expr::New(new_expr) => {
            if let ast::Expr::Ident(ident) = new_expr.callee.as_ref() {
                match ident.sym.as_ref() {
                    "Big" => Some("big.js"),
                    "Decimal" => Some("decimal.js"),
                    "BigNumber" => Some("bignumber.js"),
                    "LRUCache" => Some("lru-cache"),
                    "Command" => Some("commander"),
                    _ => None,
                }
            } else {
                None
            }
        }
        // Chained method calls: new Big(...).plus(...).div(...)
        ast::Expr::Call(call_expr) => {
            if let ast::Callee::Expr(callee_expr) = &call_expr.callee {
                if let ast::Expr::Member(member) = callee_expr.as_ref() {
                    // Recursively check the object
                    detect_native_instance_expr(&member.obj)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if a parameter pattern is a rest parameter (...args)
fn is_rest_param(pat: &ast::Pat) -> bool {
    matches!(pat, ast::Pat::Rest(_))
}

/// Extract default value from a parameter pattern (if any)
/// For optional parameters (x?: Type), we provide Expr::Undefined as the default
fn get_param_default(ctx: &mut LoweringContext, pat: &ast::Pat) -> Result<Option<Expr>> {
    match pat {
        ast::Pat::Ident(ident) => {
            // Check if this is an optional parameter (x?: Type)
            if ident.optional {
                Ok(Some(Expr::Undefined))
            } else {
                Ok(None)
            }
        }
        ast::Pat::Assign(assign) => {
            let default_expr = lower_expr(ctx, &assign.right)?;
            Ok(Some(default_expr))
        }
        _ => Ok(None),
    }
}

/// Built-in Node.js modules that are handled specially by the compiler
const BUILTIN_MODULES: &[&str] = &["fs", "path", "crypto"];

/// Check if an expression is a require() call for a built-in module.
/// Returns the module name if it is, None otherwise.
fn is_require_builtin_module(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Call(call) = expr {
        if let ast::Callee::Expr(callee_expr) = &call.callee {
            if let ast::Expr::Ident(ident) = callee_expr.as_ref() {
                if ident.sym.as_ref() == "require" {
                    // Check if the first argument is a string literal
                    if let Some(arg) = call.args.first() {
                        if let ast::Expr::Lit(ast::Lit::Str(s)) = &*arg.expr {
                            let module_name = s.value.as_str().unwrap_or("").to_string();
                            if BUILTIN_MODULES.contains(&module_name.as_str()) {
                                return Some(module_name);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Lower a destructuring assignment at statement level.
/// This allows us to properly use temporary variables for correct semantics.
/// For [a, b] = expr, we generate:
///   let __tmp = expr;
///   a = __tmp[0];
///   b = __tmp[1];
fn lower_destructuring_assignment_stmt(
    ctx: &mut LoweringContext,
    pat: &ast::AssignTargetPat,
    rhs: &ast::Expr,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    // First, evaluate and store the RHS in a temporary variable
    let rhs_expr = lower_expr(ctx, rhs)?;
    let tmp_id = ctx.fresh_local();
    let tmp_name = format!("__destruct_{}", tmp_id);
    let tmp_ty = Type::Any; // Could infer from rhs, but Any is safe
    ctx.locals.push((tmp_name.clone(), tmp_id, tmp_ty.clone()));

    result.push(Stmt::Let {
        id: tmp_id,
        name: tmp_name,
        ty: tmp_ty,
        mutable: false,
        init: Some(rhs_expr),
    });

    // Now generate assignments from the temp
    match pat {
        ast::AssignTargetPat::Array(arr_pat) => {
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    let index_expr = Expr::IndexGet {
                        object: Box::new(Expr::LocalGet(tmp_id)),
                        index: Box::new(Expr::Number(idx as f64)),
                    };

                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            if let Some(id) = ctx.lookup_local(&name) {
                                result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(index_expr))));
                            } else {
                                return Err(anyhow!(
                                    "Assignment to undeclared variable in destructuring: {}",
                                    name
                                ));
                            }
                        }
                        ast::Pat::Array(nested_arr) => {
                            // Nested array destructuring
                            // First create a temp for this element
                            let nested_tmp_id = ctx.fresh_local();
                            let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                            ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                            result.push(Stmt::Let {
                                id: nested_tmp_id,
                                name: nested_tmp_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            // Then recursively assign from it
                            let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                ctx,
                                &ast::AssignTargetPat::Array(nested_arr.clone()),
                                nested_tmp_id,
                            )?;
                            result.extend(nested_stmts);
                        }
                        ast::Pat::Object(nested_obj) => {
                            // Nested object destructuring
                            let nested_tmp_id = ctx.fresh_local();
                            let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                            ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                            result.push(Stmt::Let {
                                id: nested_tmp_id,
                                name: nested_tmp_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                ctx,
                                &ast::AssignTargetPat::Object(nested_obj.clone()),
                                nested_tmp_id,
                            )?;
                            result.extend(nested_stmts);
                        }
                        _ => {
                            // Other patterns (Rest, Expr, etc.) - skip for now
                        }
                    }
                }
            }
        }
        ast::AssignTargetPat::Object(obj_pat) => {
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            ast::PropName::Num(n) => n.value.to_string(),
                            _ => continue,
                        };

                        let prop_expr = Expr::PropertyGet {
                            object: Box::new(Expr::LocalGet(tmp_id)),
                            property: key,
                        };

                        match &*kv.value {
                            ast::Pat::Ident(ident) => {
                                let name = ident.id.sym.to_string();
                                if let Some(id) = ctx.lookup_local(&name) {
                                    result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(prop_expr))));
                                } else {
                                    return Err(anyhow!(
                                        "Assignment to undeclared variable in destructuring: {}",
                                        name
                                    ));
                                }
                            }
                            ast::Pat::Array(nested_arr) => {
                                let nested_tmp_id = ctx.fresh_local();
                                let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                                ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                                result.push(Stmt::Let {
                                    id: nested_tmp_id,
                                    name: nested_tmp_name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(prop_expr),
                                });
                                let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                    ctx,
                                    &ast::AssignTargetPat::Array(nested_arr.clone()),
                                    nested_tmp_id,
                                )?;
                                result.extend(nested_stmts);
                            }
                            ast::Pat::Object(nested_obj) => {
                                let nested_tmp_id = ctx.fresh_local();
                                let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                                ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                                result.push(Stmt::Let {
                                    id: nested_tmp_id,
                                    name: nested_tmp_name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(prop_expr),
                                });
                                let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                    ctx,
                                    &ast::AssignTargetPat::Object(nested_obj.clone()),
                                    nested_tmp_id,
                                )?;
                                result.extend(nested_stmts);
                            }
                            _ => {}
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        let name = assign.key.sym.to_string();
                        let prop_expr = Expr::PropertyGet {
                            object: Box::new(Expr::LocalGet(tmp_id)),
                            property: name.clone(),
                        };

                        if let Some(id) = ctx.lookup_local(&name) {
                            result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(prop_expr))));
                        } else {
                            return Err(anyhow!(
                                "Assignment to undeclared variable in destructuring: {}",
                                name
                            ));
                        }
                    }
                    ast::ObjectPatProp::Rest(_) => {
                        // Rest pattern - skip for now
                    }
                }
            }
        }
        ast::AssignTargetPat::Invalid(_) => {
            return Err(anyhow!("Invalid assignment target pattern"));
        }
    }

    Ok(result)
}

/// Helper for nested destructuring - assigns from an already-computed local
fn lower_destructuring_assignment_stmt_from_local(
    ctx: &mut LoweringContext,
    pat: &ast::AssignTargetPat,
    source_id: LocalId,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    match pat {
        ast::AssignTargetPat::Array(arr_pat) => {
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    let index_expr = Expr::IndexGet {
                        object: Box::new(Expr::LocalGet(source_id)),
                        index: Box::new(Expr::Number(idx as f64)),
                    };

                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            if let Some(id) = ctx.lookup_local(&name) {
                                result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(index_expr))));
                            } else {
                                return Err(anyhow!(
                                    "Assignment to undeclared variable in destructuring: {}",
                                    name
                                ));
                            }
                        }
                        ast::Pat::Array(nested_arr) => {
                            let nested_tmp_id = ctx.fresh_local();
                            let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                            ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                            result.push(Stmt::Let {
                                id: nested_tmp_id,
                                name: nested_tmp_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                ctx,
                                &ast::AssignTargetPat::Array(nested_arr.clone()),
                                nested_tmp_id,
                            )?;
                            result.extend(nested_stmts);
                        }
                        ast::Pat::Object(nested_obj) => {
                            let nested_tmp_id = ctx.fresh_local();
                            let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                            ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                            result.push(Stmt::Let {
                                id: nested_tmp_id,
                                name: nested_tmp_name,
                                ty: Type::Any,
                                mutable: false,
                                init: Some(index_expr),
                            });
                            let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                ctx,
                                &ast::AssignTargetPat::Object(nested_obj.clone()),
                                nested_tmp_id,
                            )?;
                            result.extend(nested_stmts);
                        }
                        _ => {}
                    }
                }
            }
        }
        ast::AssignTargetPat::Object(obj_pat) => {
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            ast::PropName::Num(n) => n.value.to_string(),
                            _ => continue,
                        };

                        let prop_expr = Expr::PropertyGet {
                            object: Box::new(Expr::LocalGet(source_id)),
                            property: key,
                        };

                        match &*kv.value {
                            ast::Pat::Ident(ident) => {
                                let name = ident.id.sym.to_string();
                                if let Some(id) = ctx.lookup_local(&name) {
                                    result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(prop_expr))));
                                } else {
                                    return Err(anyhow!(
                                        "Assignment to undeclared variable in destructuring: {}",
                                        name
                                    ));
                                }
                            }
                            ast::Pat::Array(nested_arr) => {
                                let nested_tmp_id = ctx.fresh_local();
                                let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                                ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                                result.push(Stmt::Let {
                                    id: nested_tmp_id,
                                    name: nested_tmp_name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(prop_expr),
                                });
                                let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                    ctx,
                                    &ast::AssignTargetPat::Array(nested_arr.clone()),
                                    nested_tmp_id,
                                )?;
                                result.extend(nested_stmts);
                            }
                            ast::Pat::Object(nested_obj) => {
                                let nested_tmp_id = ctx.fresh_local();
                                let nested_tmp_name = format!("__destruct_{}", nested_tmp_id);
                                ctx.locals.push((nested_tmp_name.clone(), nested_tmp_id, Type::Any));
                                result.push(Stmt::Let {
                                    id: nested_tmp_id,
                                    name: nested_tmp_name,
                                    ty: Type::Any,
                                    mutable: false,
                                    init: Some(prop_expr),
                                });
                                let nested_stmts = lower_destructuring_assignment_stmt_from_local(
                                    ctx,
                                    &ast::AssignTargetPat::Object(nested_obj.clone()),
                                    nested_tmp_id,
                                )?;
                                result.extend(nested_stmts);
                            }
                            _ => {}
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        let name = assign.key.sym.to_string();
                        let prop_expr = Expr::PropertyGet {
                            object: Box::new(Expr::LocalGet(source_id)),
                            property: name.clone(),
                        };

                        if let Some(id) = ctx.lookup_local(&name) {
                            result.push(Stmt::Expr(Expr::LocalSet(id, Box::new(prop_expr))));
                        } else {
                            return Err(anyhow!(
                                "Assignment to undeclared variable in destructuring: {}",
                                name
                            ));
                        }
                    }
                    ast::ObjectPatProp::Rest(_) => {}
                }
            }
        }
        ast::AssignTargetPat::Invalid(_) => {
            return Err(anyhow!("Invalid assignment target pattern"));
        }
    }

    Ok(result)
}

/// Lower a destructuring assignment expression.
/// For [a, b] = expr or { a, b } = expr, we generate a Sequence expression:
///   1. Assign each element/property to the corresponding target
///   2. Return the RHS value (assignment expressions evaluate to RHS)
///
/// Note: We reference the RHS value directly multiple times rather than
/// creating a temporary variable, since temps created in expression context
/// aren't visible to codegen. This is safe when the RHS is a simple expression
/// (which is the common case for destructuring).
fn lower_destructuring_assignment(
    ctx: &mut LoweringContext,
    pat: &ast::AssignTargetPat,
    value: Box<Expr>,
) -> Result<Expr> {
    match pat {
        ast::AssignTargetPat::Array(arr_pat) => {
            // Array destructuring assignment: [a, b] = expr
            // Desugar to:
            //   a = expr[0];
            //   b = expr[1];
            //   expr (result)
            //
            // We reference the RHS value directly. This works because:
            // 1. The RHS is typically a local variable or simple expression
            // 2. Creating a temp in expression context is problematic for codegen

            let mut exprs = Vec::new();

            // Now assign each element
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    let index_expr = Expr::IndexGet {
                        object: value.clone(),
                        index: Box::new(Expr::Number(idx as f64)),
                    };

                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            if let Some(id) = ctx.lookup_local(&name) {
                                exprs.push(Expr::LocalSet(id, Box::new(index_expr)));
                            } else {
                                return Err(anyhow!(
                                    "Assignment to undeclared variable in destructuring: {}",
                                    name
                                ));
                            }
                        }
                        ast::Pat::Expr(inner_expr) => {
                            // Expression pattern like [obj.prop] = arr
                            match inner_expr.as_ref() {
                                ast::Expr::Member(member) => {
                                    let object = Box::new(lower_expr(ctx, &member.obj)?);
                                    match &member.prop {
                                        ast::MemberProp::Ident(prop_ident) => {
                                            let property = prop_ident.sym.to_string();
                                            exprs.push(Expr::PropertySet {
                                                object,
                                                property,
                                                value: Box::new(index_expr),
                                            });
                                        }
                                        ast::MemberProp::Computed(computed) => {
                                            let index = Box::new(lower_expr(ctx, &computed.expr)?);
                                            exprs.push(Expr::IndexSet {
                                                object,
                                                index,
                                                value: Box::new(index_expr),
                                            });
                                        }
                                        _ => {
                                            return Err(anyhow!(
                                                "Unsupported member expression in destructuring"
                                            ));
                                        }
                                    }
                                }
                                _ => {
                                    return Err(anyhow!(
                                        "Unsupported expression pattern in destructuring"
                                    ));
                                }
                            }
                        }
                        ast::Pat::Rest(_) => {
                            // Rest pattern in assignment: [...rest] = arr
                            // For now, skip (would need slice operation)
                        }
                        ast::Pat::Array(nested_arr) => {
                            // Nested array destructuring: [[a, b], c] = expr
                            // Recursively lower with the indexed element as the value
                            let nested_target = ast::AssignTargetPat::Array(nested_arr.clone());
                            let nested_expr = lower_destructuring_assignment(
                                ctx,
                                &nested_target,
                                Box::new(index_expr),
                            )?;
                            exprs.push(nested_expr);
                        }
                        ast::Pat::Object(nested_obj) => {
                            // Nested object destructuring: [{ a, b }, c] = expr
                            let nested_target = ast::AssignTargetPat::Object(nested_obj.clone());
                            let nested_expr = lower_destructuring_assignment(
                                ctx,
                                &nested_target,
                                Box::new(index_expr),
                            )?;
                            exprs.push(nested_expr);
                        }
                        _ => {
                            // Other patterns (Assign with default, etc.) - skip for now
                        }
                    }
                }
                // If elem is None, it's a hole like [a, , c] - skip it
            }

            // The result of the assignment is the original RHS value
            exprs.push(*value);

            Ok(Expr::Sequence(exprs))
        }
        ast::AssignTargetPat::Object(obj_pat) => {
            // Object destructuring assignment: { a, b } = expr
            // Desugar to:
            //   a = expr.a;
            //   b = expr.b;
            //   expr (result)

            let mut exprs = Vec::new();

            // Now assign each property
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        // { key: target } - extract obj.key into target
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            ast::PropName::Num(n) => n.value.to_string(),
                            _ => continue, // Skip computed keys
                        };

                        let prop_expr = Expr::PropertyGet {
                            object: value.clone(),
                            property: key,
                        };

                        match &*kv.value {
                            ast::Pat::Ident(ident) => {
                                let name = ident.id.sym.to_string();
                                if let Some(id) = ctx.lookup_local(&name) {
                                    exprs.push(Expr::LocalSet(id, Box::new(prop_expr)));
                                } else {
                                    return Err(anyhow!(
                                        "Assignment to undeclared variable in destructuring: {}",
                                        name
                                    ));
                                }
                            }
                            ast::Pat::Array(nested_arr) => {
                                let nested_target = ast::AssignTargetPat::Array(nested_arr.clone());
                                let nested_expr = lower_destructuring_assignment(
                                    ctx,
                                    &nested_target,
                                    Box::new(prop_expr),
                                )?;
                                exprs.push(nested_expr);
                            }
                            ast::Pat::Object(nested_obj) => {
                                let nested_target =
                                    ast::AssignTargetPat::Object(nested_obj.clone());
                                let nested_expr = lower_destructuring_assignment(
                                    ctx,
                                    &nested_target,
                                    Box::new(prop_expr),
                                )?;
                                exprs.push(nested_expr);
                            }
                            _ => {
                                // Other patterns - skip for now
                            }
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        // Shorthand: { a } means { a: a }
                        let name = assign.key.sym.to_string();
                        let prop_expr = Expr::PropertyGet {
                            object: value.clone(),
                            property: name.clone(),
                        };

                        if let Some(id) = ctx.lookup_local(&name) {
                            exprs.push(Expr::LocalSet(id, Box::new(prop_expr)));
                        } else {
                            return Err(anyhow!(
                                "Assignment to undeclared variable in destructuring: {}",
                                name
                            ));
                        }
                    }
                    ast::ObjectPatProp::Rest(_) => {
                        // Rest pattern: { ...rest } - skip for now
                    }
                }
            }

            // The result of the assignment is the original RHS value
            exprs.push(*value);

            Ok(Expr::Sequence(exprs))
        }
        ast::AssignTargetPat::Invalid(_) => {
            Err(anyhow!("Invalid assignment target pattern"))
        }
    }
}

/// Lower a variable declaration, handling array destructuring patterns.
/// Returns a vector of statements (multiple for destructuring, single for simple bindings).
fn lower_var_decl_with_destructuring(
    ctx: &mut LoweringContext,
    decl: &ast::VarDeclarator,
    mutable: bool,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    match &decl.name {
        ast::Pat::Ident(ident) => {
            // Simple binding: let x = expr
            let name = ident.id.sym.to_string();
            let mut ty = ident.type_ann.as_ref()
                .map(|ann| extract_ts_type(&ann.type_ann))
                .unwrap_or(Type::Any);

            // If no type annotation, infer from new Set<T>() or new Map<K, V>() or new URLSearchParams() expressions
            if matches!(ty, Type::Any) {
                if let Some(init_expr) = &decl.init {
                    if let ast::Expr::New(new_expr) = init_expr.as_ref() {
                        if let ast::Expr::Ident(class_ident) = new_expr.callee.as_ref() {
                            let class_name = class_ident.sym.as_ref();
                            if class_name == "Set" || class_name == "Map" {
                                // Extract type arguments from new Set<T>() or new Map<K, V>()
                                let type_args: Vec<Type> = new_expr.type_args.as_ref()
                                    .map(|ta| ta.params.iter()
                                        .map(|t| extract_ts_type(t))
                                        .collect())
                                    .unwrap_or_default();
                                ty = Type::Generic {
                                    base: class_name.to_string(),
                                    type_args,
                                };
                            } else if class_name == "URLSearchParams" {
                                ty = Type::Named("URLSearchParams".to_string());
                            }
                        }
                    }
                }
            }

            // Check if this is a native class instantiation and register it
            if let Some(init_expr) = &decl.init {
                if let ast::Expr::New(new_expr) = init_expr.as_ref() {
                    if let ast::Expr::Ident(class_ident) = new_expr.callee.as_ref() {
                        let class_name = class_ident.sym.as_ref();
                        // Map class names to their modules
                        let module_name = match class_name {
                            "EventEmitter" => Some("events"),
                            "AsyncLocalStorage" => Some("async_hooks"),
                            "WebSocket" | "WebSocketServer" => Some("ws"),
                            "Redis" => Some("ioredis"),
                            "LRUCache" => Some("lru-cache"),
                            "Command" => Some("commander"),
                            "Big" => Some("big.js"),
                            "Decimal" => Some("decimal.js"),
                            "BigNumber" => Some("bignumber.js"),
                            // Database clients
                            "Pool" => Some("pg"),  // PostgreSQL connection pool
                            "Client" => Some("pg"), // PostgreSQL client
                            _ => None,
                        };
                        if let Some(module) = module_name {
                            ctx.register_native_instance(name.clone(), module.to_string(), class_name.to_string());
                        }
                    }
                }
            }

            // Check if this is an awaited native class instantiation (e.g., await new Redis())
            if let Some(init_expr) = &decl.init {
                if let ast::Expr::Await(await_expr) = init_expr.as_ref() {
                    if let ast::Expr::New(new_expr) = await_expr.arg.as_ref() {
                        if let ast::Expr::Ident(class_ident) = new_expr.callee.as_ref() {
                            let class_name = class_ident.sym.as_ref();
                            // Map class names to their modules
                            let module_name = match class_name {
                                "EventEmitter" => Some("events"),
                                "AsyncLocalStorage" => Some("async_hooks"),
                                "WebSocket" | "WebSocketServer" => Some("ws"),
                                "Redis" => Some("ioredis"),
                                "LRUCache" => Some("lru-cache"),
                                "Command" => Some("commander"),
                                "Big" => Some("big.js"),
                                "Decimal" => Some("decimal.js"),
                                "BigNumber" => Some("bignumber.js"),
                                // Database clients
                                "Pool" => Some("pg"),  // PostgreSQL connection pool
                                "Client" => Some("pg"), // PostgreSQL client
                                _ => None,
                            };
                            if let Some(module) = module_name {
                                ctx.register_native_instance(name.clone(), module.to_string(), class_name.to_string());
                            }
                        }
                    }
                }
            }

            // Check if this is a native module factory function call (e.g., mysql.createPool())
            if let Some(init_expr) = &decl.init {
                if let ast::Expr::Call(call_expr) = init_expr.as_ref() {
                    if let ast::Callee::Expr(callee) = &call_expr.callee {
                        if let ast::Expr::Member(member) = callee.as_ref() {
                            if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                                let obj_name = obj_ident.sym.as_ref();
                                // Check if it's a known native module
                                if let Some((module_name, _)) = ctx.lookup_native_module(obj_name) {
                                    if let ast::MemberProp::Ident(method_ident) = &member.prop {
                                        let method_name = method_ident.sym.as_ref();
                                        // Map factory functions to their class names
                                        let class_name = match (module_name, method_name) {
                                            ("mysql2" | "mysql2/promise", "createPool") => Some("Pool"),
                                            ("mysql2" | "mysql2/promise", "createConnection") => Some("Connection"),
                                            ("pg", "connect") => Some("Client"),
                                            _ => None,
                                        };
                                        if let Some(class_name) = class_name {
                                            ctx.register_native_instance(name.clone(), module_name.to_string(), class_name.to_string());
                                        }
                                    }
                                }
                            }
                        }

                        // Check if this is a direct call to a default import from a native module
                        // e.g., Fastify() where Fastify is imported from 'fastify'
                        if let ast::Expr::Ident(func_ident) = callee.as_ref() {
                            let func_name = func_ident.sym.as_ref();
                            // Check if this is a default import from a native module
                            if let Some((module_name, None)) = ctx.lookup_native_module(func_name) {
                                // Register as native instance - the "class" is "App" for default exports
                                ctx.register_native_instance(name.clone(), module_name.to_string(), "App".to_string());
                            }
                            // Check if this is a named import that returns a handle (e.g., State from perry/ui)
                            if let Some((module_name, Some(method_name))) = ctx.lookup_native_module(func_name) {
                                if module_name == "perry/ui" && method_name == "State" {
                                    ctx.register_native_instance(name.clone(), module_name.to_string(), "State".to_string());
                                }
                            }
                        }
                    }
                }
            }

            // Check if this is a require() call for a built-in module
            if let Some(init_expr) = &decl.init {
                if let Some(module_name) = is_require_builtin_module(init_expr) {
                    // Register this variable as an alias to the built-in module
                    ctx.register_builtin_module_alias(name.clone(), module_name);
                    // Don't emit a variable declaration - the module is handled specially
                    return Ok(result);
                }
            }

            // Check if this is calling toString() on URLSearchParams - returns String
            if matches!(ty, Type::Any) {
                if let Some(init_expr) = &decl.init {
                    if let ast::Expr::Call(call_expr) = init_expr.as_ref() {
                        if let ast::Callee::Expr(callee_expr) = &call_expr.callee {
                            if let ast::Expr::Member(member_expr) = callee_expr.as_ref() {
                                if let ast::MemberProp::Ident(method_ident) = &member_expr.prop {
                                    let method_name = method_ident.sym.as_ref();
                                    if method_name == "toString" || method_name == "get" {
                                        // Check if object is a URLSearchParams
                                        if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() {
                                            let obj_name = obj_ident.sym.as_ref();
                                            if let Some(obj_ty) = ctx.lookup_local_type(obj_name) {
                                                if matches!(obj_ty, Type::Named(name) if name == "URLSearchParams") {
                                                    ty = Type::String;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Check if this is assigning the result of a native method call that returns the same type
            // e.g., const sum = d1.plus(d2) where d1 is a Decimal -> sum should also be tracked as Decimal
            // Also handles: const r1 = new Big(...).div(...) patterns
            if let Some(init_expr) = &decl.init {
                if let ast::Expr::Call(call_expr) = init_expr.as_ref() {
                    if let ast::Callee::Expr(callee_expr) = &call_expr.callee {
                        if let ast::Expr::Member(member_expr) = callee_expr.as_ref() {
                            let mut handled = false;
                            // First try: object is an ident that's a known native instance
                            if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() {
                                let obj_name = obj_ident.sym.as_ref();
                                // Check if object is a native instance
                                if let Some((module, class)) = ctx.lookup_native_instance(obj_name) {
                                    // Check if this method returns the same type (builder pattern)
                                    if let ast::MemberProp::Ident(method_ident) = &member_expr.prop {
                                        let method_name = method_ident.sym.as_ref();
                                        // Methods that return the same type (Decimal, etc.)
                                        let returns_same_type = match class {
                                            "Decimal" | "Big" | "BigNumber" => matches!(method_name,
                                                "plus" | "minus" | "times" | "div" | "mod" |
                                                "pow" | "sqrt" | "abs" | "neg" | "round" | "floor" | "ceil"
                                            ),
                                            _ => false,
                                        };
                                        if returns_same_type {
                                            ctx.register_native_instance(name.clone(), module.to_string(), class.to_string());
                                            handled = true;
                                        }
                                    }
                                }
                            }
                            // Second try: object is new Big(...) or a chained call like new Big(...).div(...)
                            if !handled {
                                if let Some(module_name) = detect_native_instance_expr(&member_expr.obj) {
                                    let class_name = match module_name {
                                        "big.js" => "Big",
                                        "decimal.js" => "Decimal",
                                        "bignumber.js" => "BigNumber",
                                        "lru-cache" => "LRUCache",
                                        "commander" => "Command",
                                        _ => "",
                                    };
                                    if !class_name.is_empty() {
                                        ctx.register_native_instance(name.clone(), module_name.to_string(), class_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Check if this is assigning from fetch() or await fetch() - register as fetch Response
            if let Some(init_expr) = &decl.init {
                // Helper to check if an expression is a fetch call
                fn is_fetch_call(expr: &ast::Expr) -> bool {
                    if let ast::Expr::Call(call_expr) = expr {
                        if let ast::Callee::Expr(callee_expr) = &call_expr.callee {
                            if let ast::Expr::Ident(ident) = callee_expr.as_ref() {
                                return ident.sym.as_ref() == "fetch";
                            }
                        }
                    }
                    false
                }

                // Check for: const response = fetch(url)
                if is_fetch_call(init_expr) {
                    ctx.register_native_instance(name.clone(), "fetch".to_string(), "Response".to_string());
                }
                // Check for: const response = await fetch(url)
                else if let ast::Expr::Await(await_expr) = init_expr.as_ref() {
                    if is_fetch_call(&await_expr.arg) {
                        ctx.register_native_instance(name.clone(), "fetch".to_string(), "Response".to_string());
                    }
                }
            }

            let init = decl.init.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let id = ctx.define_local(name.clone(), ty.clone());
            result.push(Stmt::Let {
                id,
                name,
                ty,
                mutable,
                init,
            });
        }
        ast::Pat::Array(arr_pat) => {
            // Array destructuring: let [a, b, c] = expr
            // Desugar to:
            //   let __tmp = expr;
            //   let a = __tmp[0];
            //   let b = __tmp[1];
            //   let c = __tmp[2];

            // First, store the initializer in a temporary variable
            let init_expr = decl.init.as_ref()
                .map(|e| lower_expr(ctx, e))
                .transpose()?
                .ok_or_else(|| anyhow!("Array destructuring requires an initializer"))?;

            // Get the array type from the pattern's type annotation, or infer from init
            let arr_ty = arr_pat.type_ann.as_ref()
                .map(|ann| extract_ts_type(&ann.type_ann))
                .unwrap_or(Type::Array(Box::new(Type::Any)));

            // Determine element type
            let elem_ty = match &arr_ty {
                Type::Array(elem) => (**elem).clone(),
                Type::Tuple(types) => types.first().cloned().unwrap_or(Type::Any),
                _ => Type::Any,
            };

            // Create a temporary variable to hold the array
            let tmp_id = ctx.fresh_local();
            let tmp_name = format!("__destruct_{}", tmp_id);
            ctx.locals.push((tmp_name.clone(), tmp_id, arr_ty.clone()));

            result.push(Stmt::Let {
                id: tmp_id,
                name: tmp_name,
                ty: arr_ty.clone(),
                mutable: false,
                init: Some(init_expr),
            });

            // Now extract each element
            for (idx, elem) in arr_pat.elems.iter().enumerate() {
                if let Some(elem_pat) = elem {
                    match elem_pat {
                        ast::Pat::Ident(ident) => {
                            let name = ident.id.sym.to_string();
                            // Use explicit type annotation if provided, otherwise use inferred element type
                            let ty = ident.type_ann.as_ref()
                                .map(|ann| extract_ts_type(&ann.type_ann))
                                .unwrap_or_else(|| {
                                    // For tuples, use the specific element type
                                    match &arr_ty {
                                        Type::Tuple(types) => types.get(idx).cloned().unwrap_or(elem_ty.clone()),
                                        _ => elem_ty.clone(),
                                    }
                                });
                            let id = ctx.define_local(name.clone(), ty.clone());

                            // Generate: let name = __tmp[idx]
                            result.push(Stmt::Let {
                                id,
                                name,
                                ty,
                                mutable,
                                init: Some(Expr::IndexGet {
                                    object: Box::new(Expr::LocalGet(tmp_id)),
                                    index: Box::new(Expr::Number(idx as f64)),
                                }),
                            });
                        }
                        ast::Pat::Rest(rest_pat) => {
                            // Rest element: let [...rest] = arr
                            // This would need slice operation - for now, skip
                            if let ast::Pat::Ident(ident) = &*rest_pat.arg {
                                let name = ident.id.sym.to_string();
                                let ty = Type::Array(Box::new(elem_ty.clone()));
                                let id = ctx.define_local(name.clone(), ty.clone());
                                // For now, just assign undefined - proper slice would need runtime support
                                result.push(Stmt::Let {
                                    id,
                                    name,
                                    ty,
                                    mutable,
                                    init: Some(Expr::Undefined),
                                });
                            }
                        }
                        _ => {
                            // Nested patterns - could be nested array or object destructuring
                            // For now, skip unsupported patterns
                        }
                    }
                }
                // If elem is None, it's a hole in the pattern like [a, , c]
                // We just skip it (no variable to bind)
            }
        }
        ast::Pat::Object(obj_pat) => {
            // Object destructuring: const { a, b, c } = expr
            // Desugar to:
            //   let __tmp = expr;
            //   let a = __tmp.a;
            //   let b = __tmp.b;
            //   let c = __tmp.c;

            // First, store the initializer in a temporary variable
            let init_expr = decl.init.as_ref()
                .map(|e| lower_expr(ctx, e))
                .transpose()?
                .ok_or_else(|| anyhow!("Object destructuring requires an initializer"))?;

            // Get the object type from the pattern's type annotation
            let obj_ty = obj_pat.type_ann.as_ref()
                .map(|ann| extract_ts_type(&ann.type_ann))
                .unwrap_or(Type::Any);

            // Create a temporary variable to hold the object
            let tmp_id = ctx.fresh_local();
            let tmp_name = format!("__destruct_{}", tmp_id);
            ctx.locals.push((tmp_name.clone(), tmp_id, obj_ty.clone()));

            result.push(Stmt::Let {
                id: tmp_id,
                name: tmp_name,
                ty: obj_ty.clone(),
                mutable: false,
                init: Some(init_expr),
            });

            // Now extract each property
            for prop in &obj_pat.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        // { key: value } - extracts obj.key into variable named by value pattern
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.as_str().unwrap_or("").to_string(),
                            ast::PropName::Num(n) => n.value.to_string(),
                            _ => continue, // Skip computed keys
                        };

                        // Get the variable name from the value pattern
                        if let ast::Pat::Ident(ident) = &*kv.value {
                            let name = ident.id.sym.to_string();
                            let ty = ident.type_ann.as_ref()
                                .map(|ann| extract_ts_type(&ann.type_ann))
                                .unwrap_or(Type::Any);
                            let id = ctx.define_local(name.clone(), ty.clone());

                            // Generate: let name = __tmp.key
                            result.push(Stmt::Let {
                                id,
                                name,
                                ty,
                                mutable,
                                init: Some(Expr::PropertyGet {
                                    object: Box::new(Expr::LocalGet(tmp_id)),
                                    property: key,
                                }),
                            });
                        }
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        // { key } or { key = default } - shorthand property
                        let name = assign.key.sym.to_string();
                        let ty = assign.key.type_ann.as_ref()
                            .map(|ann| extract_ts_type(&ann.type_ann))
                            .unwrap_or(Type::Any);
                        let id = ctx.define_local(name.clone(), ty.clone());

                        // Check if there's a default value
                        let init_value = if let Some(default_expr) = &assign.value {
                            // { key = default } - use default if property is undefined
                            // Use conditional: __tmp.key !== undefined ? __tmp.key : default
                            let prop_access = Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(tmp_id)),
                                property: name.clone(),
                            };
                            let default_val = lower_expr(ctx, default_expr)?;
                            // Check if property is undefined
                            let condition = Expr::Compare {
                                op: CompareOp::Ne,
                                left: Box::new(prop_access.clone()),
                                right: Box::new(Expr::Undefined),
                            };
                            Expr::Conditional {
                                condition: Box::new(condition),
                                then_expr: Box::new(prop_access),
                                else_expr: Box::new(default_val),
                            }
                        } else {
                            // { key } - just access the property
                            Expr::PropertyGet {
                                object: Box::new(Expr::LocalGet(tmp_id)),
                                property: name.clone(),
                            }
                        };

                        result.push(Stmt::Let {
                            id,
                            name,
                            ty,
                            mutable,
                            init: Some(init_value),
                        });
                    }
                    ast::ObjectPatProp::Rest(rest) => {
                        // { ...rest } - collect remaining properties
                        // For now, just skip as this requires runtime support
                        if let ast::Pat::Ident(ident) = &*rest.arg {
                            let name = ident.id.sym.to_string();
                            let ty = Type::Any; // Rest spread type is unknown
                            let id = ctx.define_local(name.clone(), ty.clone());
                            result.push(Stmt::Let {
                                id,
                                name,
                                ty,
                                mutable,
                                init: Some(Expr::Object(vec![])), // Empty object placeholder
                            });
                        }
                    }
                }
            }
        }
        _ => {
            // For other patterns, fall back to existing behavior
            let name = get_binding_name(&decl.name)?;
            let ty = extract_binding_type(&decl.name);
            let init = decl.init.as_ref().map(|e| lower_expr(ctx, e)).transpose()?;
            let id = ctx.define_local(name.clone(), ty.clone());
            result.push(Stmt::Let {
                id,
                name,
                ty,
                mutable,
                init,
            });
        }
    }

    Ok(result)
}

/// Collect all LocalGet references from an expression
fn collect_local_refs_expr(expr: &Expr, refs: &mut Vec<LocalId>) {
    match expr {
        Expr::LocalGet(id) => refs.push(*id),
        Expr::LocalSet(id, value) => {
            refs.push(*id);
            collect_local_refs_expr(value, refs);
        }
        Expr::Binary { left, right, .. } => {
            collect_local_refs_expr(left, refs);
            collect_local_refs_expr(right, refs);
        }
        Expr::Unary { operand, .. } => {
            collect_local_refs_expr(operand, refs);
        }
        Expr::Call { callee, args, .. } => {
            collect_local_refs_expr(callee, refs);
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::IndexGet { object, index } => {
            collect_local_refs_expr(object, refs);
            collect_local_refs_expr(index, refs);
        }
        Expr::IndexSet { object, index, value } => {
            collect_local_refs_expr(object, refs);
            collect_local_refs_expr(index, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::PropertyGet { object, .. } => {
            collect_local_refs_expr(object, refs);
        }
        Expr::PropertySet { object, value, .. } => {
            collect_local_refs_expr(object, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::PropertyUpdate { object, .. } => {
            collect_local_refs_expr(object, refs);
        }
        Expr::IndexUpdate { object, index, .. } => {
            collect_local_refs_expr(object, refs);
            collect_local_refs_expr(index, refs);
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::Array(elements) => {
            for elem in elements {
                collect_local_refs_expr(elem, refs);
            }
        }
        Expr::ArraySpread(elements) => {
            for elem in elements {
                match elem {
                    ArrayElement::Expr(e) => collect_local_refs_expr(e, refs),
                    ArrayElement::Spread(e) => collect_local_refs_expr(e, refs),
                }
            }
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            collect_local_refs_expr(condition, refs);
            collect_local_refs_expr(then_expr, refs);
            collect_local_refs_expr(else_expr, refs);
        }
        Expr::Closure { body, .. } => {
            // Descend into nested closures to find transitive captures.
            // If a nested closure uses a variable from the outer scope,
            // the outer closure must also capture it to pass it down.
            for stmt in body {
                collect_local_refs_stmt(stmt, refs);
            }
        }
        Expr::Compare { left, right, .. } => {
            collect_local_refs_expr(left, refs);
            collect_local_refs_expr(right, refs);
        }
        Expr::Logical { left, right, .. } => {
            collect_local_refs_expr(left, refs);
            collect_local_refs_expr(right, refs);
        }
        Expr::GlobalGet(_) | Expr::GlobalSet(_, _) => {
            // Global variables are not captures
        }
        Expr::Object(fields) => {
            for (_, value) in fields {
                collect_local_refs_expr(value, refs);
            }
        }
        Expr::TypeOf(inner) => {
            collect_local_refs_expr(inner, refs);
        }
        Expr::InstanceOf { expr, .. } => {
            collect_local_refs_expr(expr, refs);
        }
        Expr::In { property, object } => {
            collect_local_refs_expr(property, refs);
            collect_local_refs_expr(object, refs);
        }
        Expr::Await(inner) => {
            collect_local_refs_expr(inner, refs);
        }
        Expr::Sequence(exprs) => {
            for e in exprs {
                collect_local_refs_expr(e, refs);
            }
        }
        Expr::SuperCall(args) => {
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::SuperMethodCall { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::Update { id, .. } => {
            // Update reads and writes the variable
            refs.push(*id);
        }
        // File system operations
        Expr::FsReadFileSync(path) => {
            collect_local_refs_expr(path, refs);
        }
        Expr::FsWriteFileSync(path, content) => {
            collect_local_refs_expr(path, refs);
            collect_local_refs_expr(content, refs);
        }
        Expr::FsExistsSync(path) | Expr::FsMkdirSync(path) | Expr::FsUnlinkSync(path) => {
            collect_local_refs_expr(path, refs);
        }
        Expr::FsAppendFileSync(path, content) => {
            collect_local_refs_expr(path, refs);
            collect_local_refs_expr(content, refs);
        }
        // Path operations
        Expr::PathJoin(a, b) => {
            collect_local_refs_expr(a, refs);
            collect_local_refs_expr(b, refs);
        }
        Expr::PathDirname(path) | Expr::PathBasename(path) | Expr::PathExtname(path) | Expr::PathResolve(path) => {
            collect_local_refs_expr(path, refs);
        }
        // Array methods
        Expr::ArrayPush { array_id, value } => {
            refs.push(*array_id);
            collect_local_refs_expr(value, refs);
        }
        Expr::ArrayPop(array_id) | Expr::ArrayShift(array_id) => {
            refs.push(*array_id);
        }
        Expr::ArrayUnshift { array_id, value } => {
            refs.push(*array_id);
            collect_local_refs_expr(value, refs);
        }
        Expr::ArrayIndexOf { array, value } | Expr::ArrayIncludes { array, value } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::ArraySlice { array, start, end } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(start, refs);
            if let Some(e) = end {
                collect_local_refs_expr(e, refs);
            }
        }
        Expr::ArraySplice { array_id, start, delete_count, items } => {
            refs.push(*array_id);
            collect_local_refs_expr(start, refs);
            if let Some(dc) = delete_count {
                collect_local_refs_expr(dc, refs);
            }
            for item in items {
                collect_local_refs_expr(item, refs);
            }
        }
        Expr::ArrayForEach { array, callback } | Expr::ArrayMap { array, callback } | Expr::ArrayFilter { array, callback } | Expr::ArrayFind { array, callback } | Expr::ArrayFindIndex { array, callback } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(callback, refs);
        }
        Expr::ArrayReduce { array, callback, initial } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(callback, refs);
            if let Some(init) = initial {
                collect_local_refs_expr(init, refs);
            }
        }
        Expr::ArrayJoin { array, separator } => {
            collect_local_refs_expr(array, refs);
            if let Some(sep) = separator {
                collect_local_refs_expr(sep, refs);
            }
        }
        // Native module calls
        Expr::NativeMethodCall { object, args, .. } => {
            if let Some(obj) = object {
                collect_local_refs_expr(obj, refs);
            }
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        // Static member access
        Expr::StaticFieldGet { .. } => {}
        Expr::StaticFieldSet { value, .. } => {
            collect_local_refs_expr(value, refs);
        }
        Expr::StaticMethodCall { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        // String methods
        Expr::StringSplit(string, delimiter) => {
            collect_local_refs_expr(string, refs);
            collect_local_refs_expr(delimiter, refs);
        }
        Expr::StringFromCharCode(code) => {
            collect_local_refs_expr(code, refs);
        }
        // Map operations
        Expr::MapNew => {}
        Expr::MapSet { map, key, value } => {
            collect_local_refs_expr(map, refs);
            collect_local_refs_expr(key, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::MapGet { map, key } | Expr::MapHas { map, key } | Expr::MapDelete { map, key } => {
            collect_local_refs_expr(map, refs);
            collect_local_refs_expr(key, refs);
        }
        Expr::MapSize(map) | Expr::MapClear(map) => {
            collect_local_refs_expr(map, refs);
        }
        // Set operations
        Expr::SetNew => {}
        Expr::SetAdd { set_id, value } => {
            refs.push(*set_id);
            collect_local_refs_expr(value, refs);
        }
        Expr::SetHas { set, value } | Expr::SetDelete { set, value } => {
            collect_local_refs_expr(set, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::SetSize(set) | Expr::SetClear(set) => {
            collect_local_refs_expr(set, refs);
        }
        // JSON operations
        Expr::JsonParse(expr) | Expr::JsonStringify(expr) => {
            collect_local_refs_expr(expr, refs);
        }
        // Math operations
        Expr::MathFloor(expr) | Expr::MathCeil(expr) | Expr::MathRound(expr) |
        Expr::MathAbs(expr) | Expr::MathSqrt(expr) => {
            collect_local_refs_expr(expr, refs);
        }
        Expr::MathPow(base, exp) => {
            collect_local_refs_expr(base, refs);
            collect_local_refs_expr(exp, refs);
        }
        Expr::MathMin(args) | Expr::MathMax(args) => {
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::MathRandom => {}
        // Crypto operations
        Expr::CryptoRandomBytes(expr) | Expr::CryptoSha256(expr) | Expr::CryptoMd5(expr) => {
            collect_local_refs_expr(expr, refs);
        }
        Expr::CryptoRandomUUID => {}
        // OS operations (no local refs)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsHomedir |
        Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsUptime |
        Expr::OsType | Expr::OsRelease | Expr::OsCpus | Expr::OsNetworkInterfaces |
        Expr::OsUserInfo | Expr::OsEOL => {}
        // Buffer operations
        Expr::BufferFrom { data, encoding } => {
            collect_local_refs_expr(data, refs);
            if let Some(enc) = encoding {
                collect_local_refs_expr(enc, refs);
            }
        }
        Expr::BufferAlloc { size, fill } => {
            collect_local_refs_expr(size, refs);
            if let Some(f) = fill {
                collect_local_refs_expr(f, refs);
            }
        }
        Expr::BufferAllocUnsafe(expr) | Expr::BufferConcat(expr) |
        Expr::BufferIsBuffer(expr) | Expr::BufferByteLength(expr) |
        Expr::BufferLength(expr) => {
            collect_local_refs_expr(expr, refs);
        }
        Expr::BufferToString { buffer, encoding } => {
            collect_local_refs_expr(buffer, refs);
            if let Some(enc) = encoding {
                collect_local_refs_expr(enc, refs);
            }
        }
        Expr::BufferSlice { buffer, start, end } => {
            collect_local_refs_expr(buffer, refs);
            if let Some(s) = start {
                collect_local_refs_expr(s, refs);
            }
            if let Some(e) = end {
                collect_local_refs_expr(e, refs);
            }
        }
        Expr::BufferCopy { source, target, target_start, source_start, source_end } => {
            collect_local_refs_expr(source, refs);
            collect_local_refs_expr(target, refs);
            if let Some(ts) = target_start {
                collect_local_refs_expr(ts, refs);
            }
            if let Some(ss) = source_start {
                collect_local_refs_expr(ss, refs);
            }
            if let Some(se) = source_end {
                collect_local_refs_expr(se, refs);
            }
        }
        Expr::BufferWrite { buffer, string, offset, encoding } => {
            collect_local_refs_expr(buffer, refs);
            collect_local_refs_expr(string, refs);
            if let Some(o) = offset {
                collect_local_refs_expr(o, refs);
            }
            if let Some(e) = encoding {
                collect_local_refs_expr(e, refs);
            }
        }
        Expr::BufferEquals { buffer, other } => {
            collect_local_refs_expr(buffer, refs);
            collect_local_refs_expr(other, refs);
        }
        Expr::BufferIndexGet { buffer, index } => {
            collect_local_refs_expr(buffer, refs);
            collect_local_refs_expr(index, refs);
        }
        Expr::BufferIndexSet { buffer, index, value } => {
            collect_local_refs_expr(buffer, refs);
            collect_local_refs_expr(index, refs);
            collect_local_refs_expr(value, refs);
        }
        // Child Process operations
        Expr::ChildProcessExecSync { command, options } => {
            collect_local_refs_expr(command, refs);
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs);
            }
        }
        Expr::ChildProcessSpawnSync { command, args, options } |
        Expr::ChildProcessSpawn { command, args, options } => {
            collect_local_refs_expr(command, refs);
            if let Some(a) = args {
                collect_local_refs_expr(a, refs);
            }
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs);
            }
        }
        Expr::ChildProcessExec { command, options, callback } => {
            collect_local_refs_expr(command, refs);
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs);
            }
            if let Some(cb) = callback {
                collect_local_refs_expr(cb, refs);
            }
        }
        // Net operations
        Expr::NetCreateServer { options, connection_listener } => {
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs);
            }
            if let Some(cl) = connection_listener {
                collect_local_refs_expr(cl, refs);
            }
        }
        Expr::NetCreateConnection { port, host, connect_listener } |
        Expr::NetConnect { port, host, connect_listener } => {
            collect_local_refs_expr(port, refs);
            if let Some(h) = host {
                collect_local_refs_expr(h, refs);
            }
            if let Some(cl) = connect_listener {
                collect_local_refs_expr(cl, refs);
            }
        }
        // Date operations
        Expr::DateNow => {}
        Expr::DateNew(timestamp) => {
            if let Some(ts) = timestamp {
                collect_local_refs_expr(ts, refs);
            }
        }
        Expr::DateGetTime(date) | Expr::DateToISOString(date) |
        Expr::DateGetFullYear(date) | Expr::DateGetMonth(date) | Expr::DateGetDate(date) |
        Expr::DateGetHours(date) | Expr::DateGetMinutes(date) | Expr::DateGetSeconds(date) |
        Expr::DateGetMilliseconds(date) => {
            collect_local_refs_expr(date, refs);
        }
        // URL operations
        Expr::UrlNew { url, base } => {
            collect_local_refs_expr(url, refs);
            if let Some(base_expr) = base {
                collect_local_refs_expr(base_expr, refs);
            }
        }
        Expr::UrlGetHref(url) | Expr::UrlGetPathname(url) | Expr::UrlGetProtocol(url) |
        Expr::UrlGetHost(url) | Expr::UrlGetHostname(url) | Expr::UrlGetPort(url) |
        Expr::UrlGetSearch(url) | Expr::UrlGetHash(url) | Expr::UrlGetOrigin(url) |
        Expr::UrlGetSearchParams(url) => {
            collect_local_refs_expr(url, refs);
        }
        // URLSearchParams operations
        Expr::UrlSearchParamsNew(init) => {
            if let Some(init_expr) = init {
                collect_local_refs_expr(init_expr, refs);
            }
        }
        Expr::UrlSearchParamsGet { params, name } |
        Expr::UrlSearchParamsHas { params, name } |
        Expr::UrlSearchParamsDelete { params, name } |
        Expr::UrlSearchParamsGetAll { params, name } => {
            collect_local_refs_expr(params, refs);
            collect_local_refs_expr(name, refs);
        }
        Expr::UrlSearchParamsSet { params, name, value } |
        Expr::UrlSearchParamsAppend { params, name, value } => {
            collect_local_refs_expr(params, refs);
            collect_local_refs_expr(name, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::UrlSearchParamsToString(params) => {
            collect_local_refs_expr(params, refs);
        }
        // Terminal expressions that don't contain LocalGet
        Expr::Number(_) | Expr::Integer(_) | Expr::String(_) | Expr::Bool(_) | Expr::Null |
        Expr::Undefined | Expr::BigInt(_) | Expr::This | Expr::FuncRef(_) |
        Expr::ClassRef(_) | Expr::ExternFuncRef { .. } | Expr::EnumMember { .. } |
        Expr::EnvGet(_) | Expr::ProcessUptime | Expr::ProcessCwd | Expr::ProcessMemoryUsage | Expr::NativeModuleRef(_) |
        Expr::RegExp { .. } => {}
        Expr::ObjectKeys(obj) | Expr::ObjectValues(obj) | Expr::ObjectEntries(obj) => {
            collect_local_refs_expr(obj, refs);
        }
        Expr::ArrayIsArray(value) => {
            collect_local_refs_expr(value, refs);
        }
        Expr::RegExpTest { regex, string } => {
            collect_local_refs_expr(regex, refs);
            collect_local_refs_expr(string, refs);
        }
        Expr::StringMatch { string, regex } => {
            collect_local_refs_expr(string, refs);
            collect_local_refs_expr(regex, refs);
        }
        Expr::StringReplace { string, pattern, replacement } => {
            collect_local_refs_expr(string, refs);
            collect_local_refs_expr(pattern, refs);
            collect_local_refs_expr(replacement, refs);
        }
        Expr::ParseInt { string, radix } => {
            collect_local_refs_expr(string, refs);
            if let Some(r) = radix {
                collect_local_refs_expr(r, refs);
            }
        }
        Expr::ParseFloat(string) => {
            collect_local_refs_expr(string, refs);
        }
        Expr::NumberCoerce(value) => {
            collect_local_refs_expr(value, refs);
        }
        Expr::StringCoerce(value) => {
            collect_local_refs_expr(value, refs);
        }
        Expr::IsNaN(value) => {
            collect_local_refs_expr(value, refs);
        }
        Expr::IsFinite(value) => {
            collect_local_refs_expr(value, refs);
        }
        // JS runtime expressions
        Expr::JsLoadModule { .. } => {}
        Expr::JsGetExport { module_handle, .. } => {
            collect_local_refs_expr(module_handle, refs);
        }
        Expr::JsCallFunction { module_handle, args, .. } => {
            collect_local_refs_expr(module_handle, refs);
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::JsCallMethod { object, args, .. } => {
            collect_local_refs_expr(object, refs);
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        // OS module expressions (no local refs)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsType | Expr::OsRelease |
        Expr::OsHomedir | Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsCpus => {}
        // Delete operator
        Expr::Delete(inner) => {
            collect_local_refs_expr(inner, refs);
        }
        // Error operations
        Expr::ErrorNew(msg) => {
            if let Some(m) = msg {
                collect_local_refs_expr(m, refs);
            }
        }
        Expr::ErrorMessage(err) => {
            collect_local_refs_expr(err, refs);
        }
        // Uint8Array operations
        Expr::Uint8ArrayNew(size) => {
            if let Some(s) = size {
                collect_local_refs_expr(s, refs);
            }
        }
        Expr::Uint8ArrayFrom(data) | Expr::Uint8ArrayLength(data) => {
            collect_local_refs_expr(data, refs);
        }
        Expr::Uint8ArrayGet { array, index } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(index, refs);
        }
        Expr::Uint8ArraySet { array, index, value } => {
            collect_local_refs_expr(array, refs);
            collect_local_refs_expr(index, refs);
            collect_local_refs_expr(value, refs);
        }
        // Dynamic env access
        Expr::EnvGetDynamic(key) => {
            collect_local_refs_expr(key, refs);
        }
        // JS runtime expressions with sub-expressions
        Expr::JsGetProperty { object, .. } => {
            collect_local_refs_expr(object, refs);
        }
        Expr::JsSetProperty { object, value, .. } => {
            collect_local_refs_expr(object, refs);
            collect_local_refs_expr(value, refs);
        }
        Expr::JsNew { module_handle, args, .. } => {
            collect_local_refs_expr(module_handle, refs);
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::JsNewFromHandle { constructor, args } => {
            collect_local_refs_expr(constructor, refs);
            for arg in args {
                collect_local_refs_expr(arg, refs);
            }
        }
        Expr::JsCreateCallback { closure, .. } => {
            collect_local_refs_expr(closure, refs);
        }
        // Catch-all for any other terminal expressions
        _ => {}
    }
}

/// Collect all LocalGet references from a statement
fn collect_local_refs_stmt(stmt: &Stmt, refs: &mut Vec<LocalId>) {
    match stmt {
        Stmt::Let { init, .. } => {
            if let Some(init_expr) = init {
                collect_local_refs_expr(init_expr, refs);
            }
        }
        Stmt::Expr(expr) => {
            collect_local_refs_expr(expr, refs);
        }
        Stmt::Return(expr) => {
            if let Some(e) = expr {
                collect_local_refs_expr(e, refs);
            }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_local_refs_expr(condition, refs);
            for s in then_branch {
                collect_local_refs_stmt(s, refs);
            }
            if let Some(else_stmts) = else_branch {
                for s in else_stmts {
                    collect_local_refs_stmt(s, refs);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_local_refs_expr(condition, refs);
            for s in body {
                collect_local_refs_stmt(s, refs);
            }
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(init_stmt) = init {
                collect_local_refs_stmt(init_stmt, refs);
            }
            if let Some(cond) = condition {
                collect_local_refs_expr(cond, refs);
            }
            if let Some(upd) = update {
                collect_local_refs_expr(upd, refs);
            }
            for s in body {
                collect_local_refs_stmt(s, refs);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Try { body, catch, finally } => {
            for s in body {
                collect_local_refs_stmt(s, refs);
            }
            if let Some(catch_clause) = catch {
                for s in &catch_clause.body {
                    collect_local_refs_stmt(s, refs);
                }
            }
            if let Some(finally_stmts) = finally {
                for s in finally_stmts {
                    collect_local_refs_stmt(s, refs);
                }
            }
        }
        Stmt::Switch { discriminant, cases } => {
            collect_local_refs_expr(discriminant, refs);
            for case in cases {
                if let Some(ref test) = case.test {
                    collect_local_refs_expr(test, refs);
                }
                for s in &case.body {
                    collect_local_refs_stmt(s, refs);
                }
            }
        }
        Stmt::Throw(expr) => {
            collect_local_refs_expr(expr, refs);
        }
    }
}

/// Collect all local IDs that are assigned to in a statement
fn collect_assigned_locals_stmt(stmt: &Stmt, assigned: &mut Vec<LocalId>) {
    match stmt {
        Stmt::Let { .. } => {
            // Let declaration doesn't count as assignment to outer variable
        }
        Stmt::Expr(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Stmt::Return(expr) => {
            if let Some(e) = expr {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_assigned_locals_expr(condition, assigned);
            for s in then_branch {
                collect_assigned_locals_stmt(s, assigned);
            }
            if let Some(else_stmts) = else_branch {
                for s in else_stmts {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_assigned_locals_expr(condition, assigned);
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(init_stmt) = init {
                collect_assigned_locals_stmt(init_stmt, assigned);
            }
            if let Some(cond) = condition {
                collect_assigned_locals_expr(cond, assigned);
            }
            if let Some(upd) = update {
                collect_assigned_locals_expr(upd, assigned);
            }
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
        }
        Stmt::Break | Stmt::Continue => {}
        Stmt::Try { body, catch, finally } => {
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
            if let Some(catch_clause) = catch {
                for s in &catch_clause.body {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
            if let Some(finally_stmts) = finally {
                for s in finally_stmts {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::Switch { discriminant, cases } => {
            collect_assigned_locals_expr(discriminant, assigned);
            for case in cases {
                if let Some(ref test) = case.test {
                    collect_assigned_locals_expr(test, assigned);
                }
                for s in &case.body {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::Throw(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
    }
}

/// Collect all local IDs that are assigned to in an expression
fn collect_assigned_locals_expr(expr: &Expr, assigned: &mut Vec<LocalId>) {
    match expr {
        Expr::LocalSet(id, value) => {
            // This is an assignment to a local variable
            assigned.push(*id);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::Binary { left, right, .. } | Expr::Compare { left, right, .. } | Expr::Logical { left, right, .. } => {
            collect_assigned_locals_expr(left, assigned);
            collect_assigned_locals_expr(right, assigned);
        }
        Expr::Unary { operand, .. } => {
            collect_assigned_locals_expr(operand, assigned);
        }
        Expr::Call { callee, args, .. } => {
            collect_assigned_locals_expr(callee, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::PropertyGet { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::PropertySet { object, value, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::PropertyUpdate { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::IndexGet { object, index } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::IndexSet { object, index, value } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IndexUpdate { object, index, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::Array(elements) => {
            for elem in elements {
                collect_assigned_locals_expr(elem, assigned);
            }
        }
        Expr::ArraySpread(elements) => {
            for elem in elements {
                match elem {
                    ArrayElement::Expr(e) => collect_assigned_locals_expr(e, assigned),
                    ArrayElement::Spread(e) => collect_assigned_locals_expr(e, assigned),
                }
            }
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            collect_assigned_locals_expr(condition, assigned);
            collect_assigned_locals_expr(then_expr, assigned);
            collect_assigned_locals_expr(else_expr, assigned);
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::Closure { .. } => {
            // Don't recurse into nested closures - assignments there are local to that closure
        }
        Expr::Await(inner) => {
            collect_assigned_locals_expr(inner, assigned);
        }
        Expr::Sequence(exprs) => {
            for e in exprs {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::SuperCall(args) => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::SuperMethodCall { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::Update { id, .. } => {
            // Update is an assignment
            assigned.push(*id);
        }
        // File system operations
        Expr::FsReadFileSync(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        Expr::FsWriteFileSync(path, content) => {
            collect_assigned_locals_expr(path, assigned);
            collect_assigned_locals_expr(content, assigned);
        }
        Expr::FsExistsSync(path) | Expr::FsMkdirSync(path) | Expr::FsUnlinkSync(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        Expr::FsAppendFileSync(path, content) => {
            collect_assigned_locals_expr(path, assigned);
            collect_assigned_locals_expr(content, assigned);
        }
        // Path operations
        Expr::PathJoin(a, b) => {
            collect_assigned_locals_expr(a, assigned);
            collect_assigned_locals_expr(b, assigned);
        }
        Expr::PathDirname(path) | Expr::PathBasename(path) | Expr::PathExtname(path) | Expr::PathResolve(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        // Array methods - push/unshift may reassign the array pointer
        Expr::ArrayPush { array_id, value } | Expr::ArrayUnshift { array_id, value } => {
            assigned.push(*array_id); // These may reallocate the array
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArrayPop(array_id) | Expr::ArrayShift(array_id) => {
            // These modify the array but don't reallocate
        }
        Expr::ArrayIndexOf { array, value } | Expr::ArrayIncludes { array, value } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArraySlice { array, start, end } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(start, assigned);
            if let Some(e) = end {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::ArraySplice { array_id, start, delete_count, items } => {
            assigned.push(*array_id); // Splice may reallocate the array
            collect_assigned_locals_expr(start, assigned);
            if let Some(dc) = delete_count {
                collect_assigned_locals_expr(dc, assigned);
            }
            for item in items {
                collect_assigned_locals_expr(item, assigned);
            }
        }
        Expr::ArrayForEach { array, callback } | Expr::ArrayMap { array, callback } | Expr::ArrayFilter { array, callback } | Expr::ArrayFind { array, callback } | Expr::ArrayFindIndex { array, callback } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(callback, assigned);
        }
        Expr::ArrayReduce { array, callback, initial } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(callback, assigned);
            if let Some(init) = initial {
                collect_assigned_locals_expr(init, assigned);
            }
        }
        Expr::ArrayJoin { array, separator } => {
            collect_assigned_locals_expr(array, assigned);
            if let Some(sep) = separator {
                collect_assigned_locals_expr(sep, assigned);
            }
        }
        // Native module calls
        Expr::NativeMethodCall { object, args, .. } => {
            if let Some(obj) = object {
                collect_assigned_locals_expr(obj, assigned);
            }
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // Static member access
        Expr::StaticFieldGet { .. } => {}
        Expr::StaticFieldSet { value, .. } => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::StaticMethodCall { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // String methods
        Expr::StringSplit(string, delimiter) => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(delimiter, assigned);
        }
        Expr::StringFromCharCode(code) => {
            collect_assigned_locals_expr(code, assigned);
        }
        // Map operations
        Expr::MapNew => {}
        Expr::MapSet { map, key, value } => {
            collect_assigned_locals_expr(map, assigned);
            collect_assigned_locals_expr(key, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::MapGet { map, key } | Expr::MapHas { map, key } | Expr::MapDelete { map, key } => {
            collect_assigned_locals_expr(map, assigned);
            collect_assigned_locals_expr(key, assigned);
        }
        Expr::MapSize(map) | Expr::MapClear(map) => {
            collect_assigned_locals_expr(map, assigned);
        }
        // Set operations
        Expr::SetNew => {}
        Expr::SetAdd { set_id, value } => {
            assigned.push(*set_id);  // Set is modified by add
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::SetHas { set, value } | Expr::SetDelete { set, value } => {
            collect_assigned_locals_expr(set, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::SetSize(set) | Expr::SetClear(set) => {
            collect_assigned_locals_expr(set, assigned);
        }
        // JSON operations
        Expr::JsonParse(expr) | Expr::JsonStringify(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        // Math operations
        Expr::MathFloor(expr) | Expr::MathCeil(expr) | Expr::MathRound(expr) |
        Expr::MathAbs(expr) | Expr::MathSqrt(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::MathPow(base, exp) => {
            collect_assigned_locals_expr(base, assigned);
            collect_assigned_locals_expr(exp, assigned);
        }
        Expr::MathMin(args) | Expr::MathMax(args) => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::MathRandom => {}
        // Crypto operations
        Expr::CryptoRandomBytes(expr) | Expr::CryptoSha256(expr) | Expr::CryptoMd5(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::CryptoRandomUUID => {}
        // OS operations (no assignments)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsHomedir |
        Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsUptime |
        Expr::OsType | Expr::OsRelease | Expr::OsCpus | Expr::OsNetworkInterfaces |
        Expr::OsUserInfo | Expr::OsEOL => {}
        // Buffer operations
        Expr::BufferFrom { data, encoding } => {
            collect_assigned_locals_expr(data, assigned);
            if let Some(enc) = encoding {
                collect_assigned_locals_expr(enc, assigned);
            }
        }
        Expr::BufferAlloc { size, fill } => {
            collect_assigned_locals_expr(size, assigned);
            if let Some(f) = fill {
                collect_assigned_locals_expr(f, assigned);
            }
        }
        Expr::BufferAllocUnsafe(expr) | Expr::BufferConcat(expr) |
        Expr::BufferIsBuffer(expr) | Expr::BufferByteLength(expr) |
        Expr::BufferLength(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::BufferToString { buffer, encoding } => {
            collect_assigned_locals_expr(buffer, assigned);
            if let Some(enc) = encoding {
                collect_assigned_locals_expr(enc, assigned);
            }
        }
        Expr::BufferSlice { buffer, start, end } => {
            collect_assigned_locals_expr(buffer, assigned);
            if let Some(s) = start {
                collect_assigned_locals_expr(s, assigned);
            }
            if let Some(e) = end {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::BufferCopy { source, target, target_start, source_start, source_end } => {
            collect_assigned_locals_expr(source, assigned);
            collect_assigned_locals_expr(target, assigned);
            if let Some(ts) = target_start {
                collect_assigned_locals_expr(ts, assigned);
            }
            if let Some(ss) = source_start {
                collect_assigned_locals_expr(ss, assigned);
            }
            if let Some(se) = source_end {
                collect_assigned_locals_expr(se, assigned);
            }
        }
        Expr::BufferWrite { buffer, string, offset, encoding } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(string, assigned);
            if let Some(o) = offset {
                collect_assigned_locals_expr(o, assigned);
            }
            if let Some(e) = encoding {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::BufferEquals { buffer, other } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(other, assigned);
        }
        Expr::BufferIndexGet { buffer, index } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::BufferIndexSet { buffer, index, value } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        // Child Process operations
        Expr::ChildProcessExecSync { command, options } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
        }
        Expr::ChildProcessSpawnSync { command, args, options } |
        Expr::ChildProcessSpawn { command, args, options } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(a) = args {
                collect_assigned_locals_expr(a, assigned);
            }
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
        }
        Expr::ChildProcessExec { command, options, callback } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
            if let Some(cb) = callback {
                collect_assigned_locals_expr(cb, assigned);
            }
        }
        // Net operations
        Expr::NetCreateServer { options, connection_listener } => {
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
            if let Some(cl) = connection_listener {
                collect_assigned_locals_expr(cl, assigned);
            }
        }
        Expr::NetCreateConnection { port, host, connect_listener } |
        Expr::NetConnect { port, host, connect_listener } => {
            collect_assigned_locals_expr(port, assigned);
            if let Some(h) = host {
                collect_assigned_locals_expr(h, assigned);
            }
            if let Some(cl) = connect_listener {
                collect_assigned_locals_expr(cl, assigned);
            }
        }
        // Date operations
        Expr::DateNow => {}
        Expr::DateNew(timestamp) => {
            if let Some(ts) = timestamp {
                collect_assigned_locals_expr(ts, assigned);
            }
        }
        Expr::DateGetTime(date) | Expr::DateToISOString(date) |
        Expr::DateGetFullYear(date) | Expr::DateGetMonth(date) | Expr::DateGetDate(date) |
        Expr::DateGetHours(date) | Expr::DateGetMinutes(date) | Expr::DateGetSeconds(date) |
        Expr::DateGetMilliseconds(date) => {
            collect_assigned_locals_expr(date, assigned);
        }
        // URL operations
        Expr::UrlNew { url, base } => {
            collect_assigned_locals_expr(url, assigned);
            if let Some(base_expr) = base {
                collect_assigned_locals_expr(base_expr, assigned);
            }
        }
        Expr::UrlGetHref(url) | Expr::UrlGetPathname(url) | Expr::UrlGetProtocol(url) |
        Expr::UrlGetHost(url) | Expr::UrlGetHostname(url) | Expr::UrlGetPort(url) |
        Expr::UrlGetSearch(url) | Expr::UrlGetHash(url) | Expr::UrlGetOrigin(url) |
        Expr::UrlGetSearchParams(url) => {
            collect_assigned_locals_expr(url, assigned);
        }
        // URLSearchParams operations
        Expr::UrlSearchParamsNew(init) => {
            if let Some(init_expr) = init {
                collect_assigned_locals_expr(init_expr, assigned);
            }
        }
        Expr::UrlSearchParamsGet { params, name } |
        Expr::UrlSearchParamsHas { params, name } |
        Expr::UrlSearchParamsDelete { params, name } |
        Expr::UrlSearchParamsGetAll { params, name } => {
            collect_assigned_locals_expr(params, assigned);
            collect_assigned_locals_expr(name, assigned);
        }
        Expr::UrlSearchParamsSet { params, name, value } |
        Expr::UrlSearchParamsAppend { params, name, value } => {
            collect_assigned_locals_expr(params, assigned);
            collect_assigned_locals_expr(name, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::UrlSearchParamsToString(params) => {
            collect_assigned_locals_expr(params, assigned);
        }
        // Terminal expressions that don't have children or don't assign
        Expr::LocalGet(_) | Expr::GlobalGet(_) | Expr::GlobalSet(_, _) |
        Expr::FuncRef(_) | Expr::ExternFuncRef { .. } | Expr::ClassRef(_) |
        Expr::Number(_) | Expr::Integer(_) | Expr::Bool(_) | Expr::String(_) | Expr::BigInt(_) |
        Expr::Object(_) | Expr::TypeOf(_) | Expr::InstanceOf { .. } |
        Expr::EnumMember { .. } | Expr::This | Expr::Null | Expr::Undefined |
        Expr::EnvGet(_) | Expr::ProcessUptime | Expr::ProcessCwd | Expr::ProcessMemoryUsage | Expr::NativeModuleRef(_) |
        Expr::RegExp { .. } => {}
        Expr::ObjectKeys(obj) | Expr::ObjectValues(obj) | Expr::ObjectEntries(obj) => {
            collect_assigned_locals_expr(obj, assigned);
        }
        Expr::ArrayIsArray(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::RegExpTest { regex, string } => {
            collect_assigned_locals_expr(regex, assigned);
            collect_assigned_locals_expr(string, assigned);
        }
        Expr::StringMatch { string, regex } => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(regex, assigned);
        }
        Expr::StringReplace { string, pattern, replacement } => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(pattern, assigned);
            collect_assigned_locals_expr(replacement, assigned);
        }
        Expr::ParseInt { string, radix } => {
            collect_assigned_locals_expr(string, assigned);
            if let Some(r) = radix {
                collect_assigned_locals_expr(r, assigned);
            }
        }
        Expr::ParseFloat(string) => {
            collect_assigned_locals_expr(string, assigned);
        }
        Expr::NumberCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::StringCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IsNaN(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IsFinite(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        // JS runtime expressions
        Expr::JsLoadModule { .. } => {}
        Expr::JsGetExport { module_handle, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
        }
        Expr::JsCallFunction { module_handle, args, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsCallMethod { object, args, .. } => {
            collect_assigned_locals_expr(object, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // OS module expressions (no local refs or assignments)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsType | Expr::OsRelease |
        Expr::OsHomedir | Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsCpus => {}
        // Delete operator
        Expr::Delete(inner) => {
            collect_assigned_locals_expr(inner, assigned);
        }
        // Error operations
        Expr::ErrorNew(msg) => {
            if let Some(m) = msg {
                collect_assigned_locals_expr(m, assigned);
            }
        }
        Expr::ErrorMessage(err) => {
            collect_assigned_locals_expr(err, assigned);
        }
        // Uint8Array operations
        Expr::Uint8ArrayNew(size) => {
            if let Some(s) = size {
                collect_assigned_locals_expr(s, assigned);
            }
        }
        Expr::Uint8ArrayFrom(data) | Expr::Uint8ArrayLength(data) => {
            collect_assigned_locals_expr(data, assigned);
        }
        Expr::Uint8ArrayGet { array, index } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::Uint8ArraySet { array, index, value } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        // Dynamic env access
        Expr::EnvGetDynamic(key) => {
            collect_assigned_locals_expr(key, assigned);
        }
        // JS runtime expressions with sub-expressions
        Expr::JsGetProperty { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::JsSetProperty { object, value, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::JsNew { module_handle, args, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsNewFromHandle { constructor, args } => {
            collect_assigned_locals_expr(constructor, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsCreateCallback { closure, .. } => {
            collect_assigned_locals_expr(closure, assigned);
        }
        // Catch-all for any other terminal expressions
        _ => {}
    }
}

/// Check if an expression or its children use `this`
fn uses_this_expr(expr: &Expr) -> bool {
    match expr {
        Expr::This => true,
        Expr::Binary { left, right, .. } | Expr::Compare { left, right, .. } |
        Expr::Logical { left, right, .. } => {
            uses_this_expr(left) || uses_this_expr(right)
        }
        Expr::Unary { operand, .. } => uses_this_expr(operand),
        Expr::Call { callee, args, .. } => {
            uses_this_expr(callee) || args.iter().any(uses_this_expr)
        }
        Expr::PropertyGet { object, .. } | Expr::PropertyUpdate { object, .. } => {
            uses_this_expr(object)
        }
        Expr::PropertySet { object, value, .. } => {
            uses_this_expr(object) || uses_this_expr(value)
        }
        Expr::IndexGet { object, index } => {
            uses_this_expr(object) || uses_this_expr(index)
        }
        Expr::IndexSet { object, index, value } => {
            uses_this_expr(object) || uses_this_expr(index) || uses_this_expr(value)
        }
        Expr::LocalSet(_, value) => uses_this_expr(value),
        Expr::New { args, .. } => args.iter().any(uses_this_expr),
        Expr::Array(elements) => elements.iter().any(uses_this_expr),
        Expr::ArraySpread(elements) => elements.iter().any(|e| match e {
            ArrayElement::Expr(e) | ArrayElement::Spread(e) => uses_this_expr(e),
        }),
        Expr::Object(fields) => fields.iter().any(|(_, e)| uses_this_expr(e)),
        Expr::Conditional { condition, then_expr, else_expr } => {
            uses_this_expr(condition) || uses_this_expr(then_expr) || uses_this_expr(else_expr)
        }
        Expr::Await(inner) => uses_this_expr(inner),
        Expr::Sequence(exprs) => exprs.iter().any(uses_this_expr),
        Expr::NativeMethodCall { object, args, .. } => {
            object.as_ref().map(|o| uses_this_expr(o)).unwrap_or(false) || args.iter().any(uses_this_expr)
        }
        Expr::SuperCall(args) | Expr::SuperMethodCall { args, .. } => args.iter().any(uses_this_expr),
        Expr::Closure { .. } => {
            // Don't recurse into nested closures - they have their own `this` handling
            false
        }
        // Terminal expressions that don't use `this`
        _ => false,
    }
}

/// Check if a statement or its children use `this`
fn uses_this_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init: Some(expr), .. } => uses_this_expr(expr),
        Stmt::Expr(expr) => uses_this_expr(expr),
        Stmt::Return(Some(expr)) => uses_this_expr(expr),
        Stmt::If { condition, then_branch, else_branch } => {
            uses_this_expr(condition) ||
            then_branch.iter().any(uses_this_stmt) ||
            else_branch.as_ref().map(|b| b.iter().any(uses_this_stmt)).unwrap_or(false)
        }
        Stmt::While { condition, body } => {
            uses_this_expr(condition) || body.iter().any(uses_this_stmt)
        }
        Stmt::For { init, condition, update, body } => {
            init.as_ref().map(|s| uses_this_stmt(s)).unwrap_or(false) ||
            condition.as_ref().map(|e| uses_this_expr(e)).unwrap_or(false) ||
            update.as_ref().map(|e| uses_this_expr(e)).unwrap_or(false) ||
            body.iter().any(uses_this_stmt)
        }
        Stmt::Try { body, catch, finally } => {
            body.iter().any(uses_this_stmt) ||
            catch.as_ref().map(|c| c.body.iter().any(uses_this_stmt)).unwrap_or(false) ||
            finally.as_ref().map(|f| f.iter().any(uses_this_stmt)).unwrap_or(false)
        }
        Stmt::Throw(expr) => uses_this_expr(expr),
        Stmt::Switch { discriminant, cases } => {
            uses_this_expr(discriminant) ||
            cases.iter().any(|c| {
                c.test.as_ref().map(uses_this_expr).unwrap_or(false) ||
                c.body.iter().any(uses_this_stmt)
            })
        }
        _ => false,
    }
}

/// Check if a closure body uses `this`
fn closure_uses_this(body: &[Stmt]) -> bool {
    body.iter().any(uses_this_stmt)
}

/// Check if a name is a built-in global function provided by the runtime
fn is_builtin_function(name: &str) -> bool {
    matches!(name, "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval" | "fetch")
}
