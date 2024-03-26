use bumpalo::{collections::string::String, Bump};
use const_format::formatcp;
use core::{
    fmt::{self, Write},
    iter,
};
use roc_docs_types::TypeAnnotation; // TODO move this into its own no_std, no deps crate
use roc_module::symbol::{IdentId, ModuleId}; // TODO move these into their own no_std, no deps crates
use roc_types::{
    subs::Variable,                  // TODO move this into its own no_std, no deps crate
    types::{Alias, AliasKind, Type}, // TODO move this to top-level roc_types, and make it no_std and no deps
};

/// A named heading in the sidebar, with some number of
/// entries beneath it.
pub struct SidebarEntry<'a, StrIter> {
    /// In the source code, this will appear in a module's `exposes` list like:
    ///
    /// [
    ///     Foo,
    ///     Bar,
    ///     ## Heading
    ///     Baz,
    ///     Blah,
    /// ]
    pub link_text: &'a str,

    /// The entries this module exposes (types, values, abilities)
    pub exposed: StrIter,

    /// These doc comments get interpreted as flat strings; Markdown is not allowed
    /// in them, because they will be rendered in the sidebar as plain text.
    pub doc_comment: Option<&'a str>,
}

pub struct RecordField<'a> {
    field_name: &'a str,
    value_type: &'a Type,
    is_required: bool,
}

pub struct BodyEntry<'a, IdentId> {
    pub entry_name: &'a str,
    pub ident_id: IdentId,
    pub type_vars_names: &'a [&'a str],
    pub type_annotation: TypeAnnotation,
    pub docs: Option<&'a str>,
}

pub trait Docs<
    'a,
    ModuleNames: Iterator<Item = &'a (ModuleId, &'a str)>,
    Sidebar: Iterator<Item = SidebarEntry<'a, StrIter>>,
    StrIter: Iterator<Item = &'a str>,
    BodyEntries: Iterator<Item = BodyEntry<'a, IdentId>>,
>
{
    // Required constants
    fn package_name(&self) -> &'a str;
    fn user_specified_base_url(&self) -> Option<&'a str>;
    fn raw_template_html(&self) -> &'a str;
    fn package_doc_comment_html(&self) -> &'a str;

    // Required iterators
    fn module_names(&self) -> ModuleNames;
    fn package_sidebar_entries(&self) -> Sidebar;
    fn body_entries(&self) -> BodyEntries;

    // Required lookups
    fn base_url(&self, module_id: ModuleId) -> &'a str;
    fn module_name(&self, module_id: ModuleId) -> &'a str;
    fn ident_name(&self, module_id: ModuleId, ident_id: IdentId) -> &'a str;
    fn opt_type(
        &self,
        module_id: ModuleId,
        ident_id: IdentId,
    ) -> Option<Result<&'a Type, Variable>>;
    fn opt_alias(&self, module_id: ModuleId, ident_id: IdentId) -> Option<&'a Alias>;

    // Implementation
    fn render_to_disk<Problem>(
        &self,
        arena: &'a Bump,
        // Takes the module name to be used as the directory name (or None if this is the root index.html),
        // as well as the contents of the file.
        write_to_disk: impl Fn(Option<&str>, &str) -> Result<(), Problem>,
    ) -> Result<(), Problem> {
        let package_doc_comment_html = self.package_doc_comment_html();
        let raw_template_html = self.raw_template_html();
        let package_name = self.package_name();
        let mut buf = String::with_capacity_in(raw_template_html.len() + 2048, arena);
        let mut module_template_html =
            String::with_capacity_in(raw_template_html.len() + 2048, arena);
        let mut sidebar_links = String::with_capacity_in(4096, arena);

        let sidebar_links = &mut sidebar_links;

        self.render_sidebar(sidebar_links);

        // Write index.html for package (/index.html)
        {
            let mut src = raw_template_html;

            {
                src = advance_past("<!-- base -->", src, &mut buf);
                write_base_url(self.user_specified_base_url(), &mut buf);
            }

            {
                src = advance_past("<!-- Prefetch links -->", src, &mut buf);

                for (index, (_, module_name)) in self.module_names().enumerate() {
                    if index > 0 {
                        buf.push_str("\n    ");
                    }

                    let _ = write!(buf, "<link rel='prefetch' href='{module_name}'/>",);
                }
            }

            // Set module_template_html to be all the replacements we've made so far,
            // plus the rest of the source template. We'll use this partially-completed
            // template later on for the individual modules.
            {
                module_template_html.push_str(&buf);
                module_template_html.push_str(&src);
            }

            {
                src = advance_past("<!-- Page title -->", src, &mut buf);
                let _ = write!(buf, "<title>{package_name}</title>");
            }

            {
                src = advance_past("<!-- Module links -->", src, &mut buf);
                buf.push_str(&sidebar_links);
            }

            {
                src = advance_past("<!-- Package Name -->", src, &mut buf);
                render_package_name_link(package_name, &mut buf);
            }

            {
                src = advance_past("<!-- Module Docs -->", src, &mut buf);

                if package_doc_comment_html.is_empty() {
                    buf.push_str("Choose a module from the list to see its documentation.");
                } else {
                    buf.push_str(package_doc_comment_html);
                }
            }

            {
                // Write the rest of the template into the buffer.
                buf.push_str(&src);

                // Finally, write the accumulated buffer to disk.
                write_to_disk(None, &buf)?;

                buf.clear(); // We're done with this now. It's ready to be reused!
            }
        }

        // Write each package module's index.html file
        for (module_id, module_name) in self.module_names() {
            let mut src = module_template_html.as_str();

            {
                {
                    src = advance_past("<!-- Page title -->", src, &mut buf);
                    let _ = write!(buf, "<title>{module_name} - {package_name}</title>",);
                }

                {
                    src = advance_past("<!-- Module links -->", src, &mut buf);
                    buf.push_str(sidebar_links);
                }

                {
                    src = advance_past("<!-- Package Name -->", src, &mut buf);
                    render_package_name_link(package_name, &mut buf);
                }
            }

            {
                src = advance_past("<!-- Module Docs -->", src, &mut buf);
                self.render_module(arena, *module_id, &mut buf);
            }

            {
                // Write the rest of the template into the buffer.
                buf.push_str(&src);
            }

            {
                // Finally, write the accumulated buffer to disk.
                write_to_disk(Some(module_name), &buf)?;
            }

            buf.clear(); // We're done with this now. It's ready to be reused in the next iteration of the loop!
        }

        Ok(())
    }

    fn render_sidebar(&self, buf: &mut String<'_>) {
        for SidebarEntry {
            link_text: module_name,
            doc_comment,
            exposed,
        } in self.package_sidebar_entries()
        {
            if let Some(heading) = doc_comment {
                let _ = write!(buf, "\t<h3 class=\"sidebar-heading\">{heading}</a>\n");
            }

            // Sidebar entries should all be relative URLs and unqualified names
            let _ = write!(
                buf,
                "<div class='sidebar-entry'><a class='sidebar-module-link' href='{module_name}'>{module_name}</a><div class='sidebar-sub-entries'>",
            );

            for name in exposed {
                let _ = write!(buf, "<a href='{module_name}#{name}'>{name}</a>",);
            }

            buf.push_str("</div></div>");
        }
    }

    fn render_type(
        &self,
        buf: &mut String<'_>,
        indent: Indentation,
        typ: &'a Type,
        // Whether the type needs to be wrapped in parens (only matters if the rendered type contains spaces,
        // e.g. function application or tags with payloads)
        _wrap_in_parens: WrapInParens,
    ) {
        use Type::*;

        let todo = (); // TODO use wrap_in_parens

        match typ {
            EmptyRec => self.render_record_type(buf, indent, iter::empty()),
            EmptyTagUnion => self.render_tag_union_type(buf, indent, iter::empty()),
            Function(args, _closure_size, ret) => {
                if is_multiline(typ) {
                    buf.push_str("(\n");
                    self.render_function_type(buf, indent.increment(), args.iter(), &*ret);
                    let _ = write!(buf, "{indent}(\n");
                } else {
                    buf.push_str("(");
                    self.render_function_type(buf, indent, args.iter(), &*ret);
                    buf.push_str(")");
                }
            }
            Record(fields, _ext) => self.render_record_type(
                buf,
                indent,
                fields.iter().map(|(field_name, field)| {
                    use roc_types::types::RecordField::*;

                    match field {
                        Required(typ) | RigidRequired(typ) | Demanded(typ) => RecordField {
                            field_name: field_name.as_str(),
                            value_type: typ,
                            is_required: true,
                        },
                        Optional(typ) | RigidOptional(typ) => RecordField {
                            field_name: field_name.as_str(),
                            value_type: typ,
                            is_required: false,
                        },
                    }
                }),
            ),
            Tuple(_, _) => todo!(),
            TagUnion(tags, _ext) => self.render_tag_union_type(
                buf,
                indent,
                tags.iter()
                    .map(|(tag_name, payloads)| (tag_name.0.as_str(), payloads.as_slice())),
            ),
            FunctionOrTagUnion(_, _, _) => todo!(),
            ClosureTag {
                name: _,
                captures: _,
                ambient_function: _,
            } => todo!(),
            UnspecializedLambdaSet { unspecialized: _ } => todo!(),
            DelayedAlias(_) => todo!(),
            Alias {
                symbol: _,
                type_arguments: _,
                lambda_set_variables: _,
                infer_ext_in_output_types: _,
                actual: _,
                kind: _,
            } => todo!(),
            RecursiveTagUnion(_, _, _) => todo!(),
            Apply(_, _, _) => todo!(),
            Variable(_) => todo!(),
            RangedNumber(_) => todo!(),
            Error => todo!(),
        }
    }

    fn render_record_type(
        &self,
        buf: &mut String<'_>,
        indent: Indentation,
        mut fields: impl ExactSizeIterator<Item = RecordField<'a>>,
    ) {
        const BRACES_CLASS_NAME: &str = "literal";
        const OPEN_BRACE_HTML: &str = formatcp!("<span class='{BRACES_CLASS_NAME}'>{{</span>");
        const CLOSE_BRACE_HTML: &str = formatcp!("<span class='{BRACES_CLASS_NAME}'>}}</span>");

        match fields.next() {
            None => {
                // Empty records are just "{}"
                let _ = write!(buf, "<span class='{BRACES_CLASS_NAME}'>{{}}</span>");
            }
            Some(RecordField {
                field_name,
                value_type,
                is_required,
            }) if fields.len() == 1 && !is_multiline(value_type) => {
                let colon_or_question_mark = if is_required { ":" } else { "?" };

                // If the record has one field, and that field's value is single-line,
                // then we print the record on one line with spaces inside the braces
                let _ = write!(
                    buf,
                    "{OPEN_BRACE_HTML} {field_name} {colon_or_question_mark} "
                );
                self.render_type(buf, indent, value_type, WrapInParens::Unnecessary);
                let _ = write!(buf, " {CLOSE_BRACE_HTML}");
            }
            Some(first) => {
                // Multi-field records are on multiple lines, with each line indented and ending in a trailing comma
                let _ = write!(buf, "{indent}{OPEN_BRACE_HTML}");

                {
                    // Indent one extra level while we're inside the braces.
                    let indent = indent.increment();

                    for RecordField {
                        field_name,
                        value_type,
                        is_required,
                    } in iter::once(first).chain(fields)
                    {
                        let colon_or_question_mark = if is_required { ":" } else { "?" };

                        let _ = write!(buf, "{indent}{field_name} {colon_or_question_mark} ");

                        if is_multiline(value_type) {
                            buf.push_str("\n");
                        } else {
                            buf.push_str(" ");
                        }

                        self.render_type(buf, indent, value_type, WrapInParens::Unnecessary);

                        // Put a trailing comma at the end of each line.
                        buf.push_str(",");
                    }
                }

                // The closing brace goes on its own line, indented.
                let _ = write!(buf, "{indent}{CLOSE_BRACE_HTML}");
            }
        }
    }

    fn render_tag_union_type(
        &self,
        buf: &mut String<'_>,
        indent: Indentation,
        mut tags: impl ExactSizeIterator<Item = (&'a str, &'a [Type])>,
    ) {
        const BRACES_CLASS_NAME: &str = "literal";
        const TAG_CLASS_NAME: &str = "literal";
        const OPEN_BRACE_HTML: &str = formatcp!("<span class='{BRACES_CLASS_NAME}'>[</span>");
        const CLOSE_BRACE_HTML: &str = formatcp!("<span class='{BRACES_CLASS_NAME}'>]</span>");

        match tags.next() {
            None => {
                // Empty tag unions are just "[]"
                let _ = write!(buf, "<span class='{BRACES_CLASS_NAME}'>[]</span>");
            }
            Some((tag, payloads)) if tags.len() == 1 && !payloads.iter().any(is_multiline) => {
                // Single-line tag unions don't have spaces inside the braces
                let _ = write!(
                    buf,
                    "{OPEN_BRACE_HTML}<span class='{TAG_CLASS_NAME}'>{tag}</span>"
                );

                for typ in payloads.iter() {
                    buf.push_str(" ");
                    self.render_type(buf, indent, typ, WrapInParens::NeededIfWhitespace);
                }

                buf.push_str(CLOSE_BRACE_HTML);
            }
            Some(first) => {
                // Multi-tag unions are on multiple lines, with each line indented and ending in a trailing comma
                let _ = write!(buf, "{indent}{OPEN_BRACE_HTML}");

                {
                    // Indent one extra level while we're inside the braces.
                    let indent = indent.increment();

                    for (tag, payloads) in iter::once(first).chain(tags) {
                        let _ = write!(buf, "{indent}<span class='{TAG_CLASS_NAME}'>{tag}</span>");

                        for typ in payloads.iter() {
                            buf.push_str(" ");
                            self.render_type(buf, indent, typ, WrapInParens::NeededIfWhitespace);
                        }

                        // Put a trailing comma at the end of each line.
                        let _ = buf.push_str(",");
                    }
                }

                // The closing brace goes on its own line, indented.
                let _ = write!(buf, "{indent}{CLOSE_BRACE_HTML}");
            }
        }
    }

    fn render_function_type(
        &self,
        buf: &mut String<'_>,
        indent: Indentation,
        mut args: impl ExactSizeIterator<Item = &'a Type>,
        ret: &'a Type,
    ) {
        let args_len = args.len();

        // Render args as multiline if the function has more than 3 args, or if any args are multiline
        if args_len > 3 || args.any(is_multiline) {
            let indent = indent.increment();

            for (index, arg) in args.enumerate() {
                let _ = write!(buf, "\n{indent}");

                self.render_type(buf, indent, arg, WrapInParens::Unnecessary);

                if index < args_len - 1 {
                    // Put a comma at the end of each line except the last one,
                    // because the -> is next.
                    buf.push_str(",");
                }
            }

            let _ = write!(buf, "\n{indent}->");
        } else {
            for (index, arg) in args.enumerate() {
                self.render_type(buf, indent, arg, WrapInParens::Unnecessary);

                if index < args_len - 1 {
                    // Put a comma at the end of each line except the last one,
                    // because the -> is next.
                    buf.push_str(", ");
                }
            }

            buf.push_str(" ->");
        }

        let indent = if is_multiline(ret) {
            let _ = write!(buf, "\n{indent}");

            indent.increment()
        } else {
            buf.push_str(" ");

            indent
        };

        self.render_type(buf, indent, ret, WrapInParens::Unnecessary);
    }

    fn render_absolute_url(&self, ident_id: IdentId, module_id: ModuleId, buf: &mut String<'_>) {
        let base_url = self.base_url(module_id);

        let _ = write!(
            buf,
            // e.g. "https://example.com/Str#isEmpty"
            "{base_url}{}#{}",
            self.module_name(module_id),
            self.ident_name(module_id, ident_id)
        );
    }

    fn render_module(&self, arena: &'a Bump, module_id: ModuleId, buf: &mut String<'_>) {
        let indent = Indentation::default();
        let module_name = self.module_name(module_id);
        let _ = write!(
            buf,
            "<h2 class='module-name'><a href='/{module_name}'>{module_name}</a></h2>"
        );

        for entry in self.body_entries() {
            let name = entry.entry_name;
            let ident_id = entry.ident_id;
            let type_ann = entry.type_annotation;

            let _ = write!(
                        buf,
                        "<section><h3 id='{name}' class='entry-name'><a href='{module_name}#{name}'>{name}</a>"
                    );

            if matches!(type_ann, TypeAnnotation::Ability { .. }) {
                // Ability declarations don't have ":" after the name, just `implements`
                buf.push_str(" <span class='kw'>implements</span>");
                let todo = (); // TODO render ability declaration here
            } else if let Some(mut alias) = self.opt_alias(module_id, ident_id) {
                // This is a type entry (either a type alias or an opaque type)

                // Print all the variables in the type right after the name,
                // separated by spaces - e.g. the `ok` and `err` in:
                //
                //     Result ok err :
                for loc_alias_var in alias.type_variables.iter() {
                    let _ = write!(buf, " {}", loc_alias_var.value.name.as_str());
                }

                // Resolve as many aliases as necessary
                loop {
                    match alias.kind {
                        AliasKind::Structural => {
                            // If this is an alias of another alias, inline the other alias so you can
                            // see what the actual underlying type is.
                            //
                            // DESIGN NOTE: in the future, we might want to do this only when
                            // this alias resolves to another alias which is in an unexposed module,
                            // e.g. the alias Http.Request is exposed, but it's an alias to
                            // InternalHttp.Request, which is not exposed because the InternalHttp
                            // module is not exposed. (In that case, it's very important that we
                            // inline the annotation because otherwise you just don't see anything,
                            // and you can't tell what the alias is aliasing without inducing a type
                            // mismatch, reading the source code, asking editor tooling to infer it, etc.)
                            if let Type::DelayedAlias(alias_common) = &alias.typ {
                                if let Some(new_alias) = self.opt_alias(
                                    alias_common.symbol.module_id(),
                                    alias_common.symbol.ident_id(),
                                ) {
                                    alias = new_alias;
                                    continue;
                                }
                            }

                            match type_ann {
                                TypeAnnotation::NoTypeAnn => {
                                    let todo = (); // TODO if this turns out to be an alias of an internal opaque type (after expansion), do the AliasKind::Opaque logic instead (including not printing ":")

                                    dbg!("alias", name, &alias.typ);
                                }
                                _ann => {
                                    dbg!("body", name, &_ann);
                                    let todo = (); // TODO this is the body; actually render each of the other types into HTML here!
                                }
                            }
                            buf.push_str(" <span class='kw'>:</span>");

                            let todo = (); // TODO render the alias body, including the logic for expanding InternalPath etc.
                        }
                        AliasKind::Opaque => {
                            // We print `:` for type aliases, but print nothing for opaque types
                            // because we don't expose the internal structure of opaque types.

                            let todo = (); // TODO print `implements` for this opaque type, if it implements any abilities
                        }
                    }

                    // By default, break here. We only `continue` earlier on if we need to continue resolving an alias.
                    break;
                }

                // If we have any ability restrictions on the type alias variables, print them at the end.
                let num_bound_vars = alias.type_variables.iter().fold(0, |count, loc_alias_var| {
                    count + loc_alias_var.value.opt_bound_abilities.is_some() as usize
                });

                if num_bound_vars > 0 {
                    let _ = write!(buf, "{indent}<span class='kw'>where</span>");

                    // if there are multiple variables, print each variable on its own line.
                    let is_multiline = num_bound_vars > 1;

                    let indent = if is_multiline {
                        indent.increment()
                    } else {
                        indent
                    };

                    for loc_alias_var in alias.type_variables.iter() {
                        if let Some(ability_set) = &loc_alias_var.value.opt_bound_abilities {
                            let type_var = loc_alias_var.value.name.as_str();

                            if is_multiline {
                                let _ = write!(buf, "{indent}");
                            } else {
                                buf.push_str(" ");
                            };

                            let _ = write!(
                                buf,
                                "<span class='type-var'>{type_var}</span> <span class='kw'>implements</span> "
                            );

                            for (index, symbol) in ability_set.sorted_iter().enumerate() {
                                if index > 0 {
                                    buf.push_str("&amp; ");
                                }

                                let ident_id = symbol.ident_id();
                                let module_id = symbol.module_id();
                                let (ability_name, todo) = ("<todo>", ()); // TODO get IdentIds for this module_id and use that to print the ident_id

                                let todo = (); // TODO make this <a> link to the Ability's docs
                                let _ = write!(
                                    buf,
                                    "<a class='ability' href='#todo'>{ability_name}</a>",
                                );
                            }

                            // Put trailing commas at the end of each `implements` line
                            if is_multiline {
                                buf.push_str(",");
                            }
                        }
                    }
                }
            } else if let Some(ann_result) = self.opt_type(module_id, ident_id) {
                buf.push_str(" <span class='kw'>:</span>");

                // This is a value entry (either a function or a non-function constant)
                match ann_result {
                    Ok(ann) => {
                        // dbg!("decl ann", &ann.signature);
                    }
                    Err(var) => {
                        // dbg!("decl var", var);
                    }
                }
            } else {
                // We should always have a variable, but if we don't, then in release builds
                // we gracefully recover by not rendering a type. In debug builds, we panic.
                #[cfg(debug_assertions)]
                {
                    unreachable!("Tried to render docs for IdentId ({:?}) in module {module_name} which had no corresponding Variable. This should never happen!", ident_id);
                }
            }

            buf.push_str("</h3>");

            if let Some(doc_str) = entry.docs {
                let todo = (); // TODO render markdown
                buf.push_str(doc_str);
            }

            buf.push_str("</section>");
        }

        // for (
        //     var,
        // ) in exposed
        // {
        //     let _ = write!(
        //         buf,
        //         "<section><h3 id='{name}' class='entry-name'><a href='{module_name}#{name}'>{name}</a> :"
        //     );

        //     // match typ {
        //     //     Type::Alias {
        //     //         kind: AliasKind::Opaque,
        //     //         ..
        //     //     } => {
        //     //         buf.push_str(":= ");
        //     //         self.render_type(buf, Indentation::default(), typ, WrapInParens::Unnecessary)
        //     //     }
        //     //     // If this decl is just type alais to a type from another module (commonly `Foo : InternalFoo`),
        //     //     // then render the actual type rather than linking to the other module's alias.
        //     //     //
        //     //     // We could make the rule be "only do this if the linked type is not exposed" but that's more
        //     //     // complicated than the simpler rule of "if it's an alias of another type, always render the
        //     //     // final type it aliases to," and we can always adjust later if there turns out to be some
        //     //     // use case where that's not the behavior we want.
        //     //     Type::Alias {
        //     //         kind: AliasKind::Structural,
        //     //         symbol,
        //     //         actual,
        //     //         ..
        //     //     } if symbol.module_id() != self.home => {
        //     //         buf.push_str(": ");
        //     //         self.render_type(
        //     //             buf,
        //     //             Indentation::default(),
        //     //             actual,
        //     //             WrapInParens::Unnecessary,
        //     //         )
        //     //     }
        //     //     typ => {
        //     //         buf.push_str(": ");
        //     //         self.render_type(buf, Indentation::default(), typ, WrapInParens::Unnecessary)
        //     //     }
        //     // }

        //     buf.push_str("</section>");
        // }
    }
}

pub fn render_package_name_link(name: &str, buf: &mut String<'_>) {
    let _ = write!(buf, "<h1 class='pkg-full-name'><a href='/'>{name}</a></h1>");
}

fn is_multiline(_first: &Type) -> bool {
    let todo = ();

    true
}

fn advance_past<'a>(needle: &'static str, src: &'a str, buf: &mut String<'_>) -> &'a str {
    if let Some(start_index) = src.find(needle) {
        // Copy over everything up to this point.
        buf.push_str(&src[..start_index]);

        // Advance past the end of this string.
        &src[(start_index + needle.len())..]
    } else {
        unreachable!( // TODO replace this with a panic in debug builds and a much more concise crash in release
            "Compiler bug in docs generation code: could not find doc template section {:?} in the template - this should never happen!\n\nNOTE: advance_past must be called on each template section in the order they appear in the template! This improves performance, but means that working on sections out of order can lead to this error.\n\nAt this point, the remaining template was:\n\n{src}",
            needle
        );
    }
}

fn write_base_url(user_specified_base_url: Option<impl AsRef<str>>, buf: &mut String) {
    // e.g. "builtins/" in "https://roc-lang.org/builtins/Str"
    match user_specified_base_url {
        Some(root_builtins_path) => {
            let root_builtins_path = root_builtins_path.as_ref();

            if !root_builtins_path.starts_with('/') {
                buf.push('/');
            }

            buf.push_str(&root_builtins_path);

            if !root_builtins_path.ends_with('/') {
                buf.push('/');
            }
        }
        None => {
            buf.push('/');
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Indentation {
    level: u32,
}

#[derive(Clone, Copy)]
enum WrapInParens {
    Unnecessary,
    NeededIfWhitespace,
}

impl Indentation {
    const INDENT_STR: &str = "    ";

    pub fn increment(self) -> Self {
        Self {
            level: self.level.saturating_add(1),
        }
    }

    pub fn decrement(self) -> Self {
        Self {
            level: self.level.saturating_sub(1),
        }
    }
}

impl fmt::Display for Indentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Always start with a newline before indenting
        f.write_char('\n')?;

        for _ in 0..self.level {
            f.write_str(Self::INDENT_STR)?;
        }

        Ok(())
    }
}
