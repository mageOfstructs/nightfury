#![feature(let_chains)]

use std::io::Write;

use console::Term;
use lib::{frontend::do_stuff, *};
use regex::Regex;

fn main() {
    // let ebnf = r"
    //     syntax ::= ( signed_keyword )? types value;
    //     signed_keyword ::= 'signed' | 'unsigned';
    //     types ::= 'int' | 'short';
    //     value ::= #'^.+;$';
    // ";
    let ebnf = r"
        list ::= #'[0-9]' ( ',' list )?;
    ";
    let ebnf = r"
        query ::= select | insert;
        select ::= 'SELECT' collist 'FROM' #'^.*;$';
        insert ::= 'INSERT INTO' #'^.* $' 'VALUES' '(' collist2 ')';
        collist ::= col ( ',' collist )?;
        collist2 ::= col2 ( ',' collist2 )?;
        col ::= #'^.*[, ]$' | '*';
        col2 ::= #'^.*[, ]$' | '*';
    ";
    let sql = r##"
    sql ::= statement { statement };

statement ::= (select_statement | insert_statement | update_statement | delete_statement | create_statement | drop_statement | alter_statement | transaction_statement) ';';

select_statement ::= "SELECT" select_list "FROM" table_reference [ where_clause ] [ group_by_clause ] [ order_by_clause ];

select_list ::= "*" | ( column_name { "," column_name } );

table_reference ::= table_name [ alias ] { "," table_name [ alias ] };

where_clause ::= "WHERE" condition;

group_by_clause ::= "GROUP BY" column_name { "," column_name };

order_by_clause ::= "ORDER BY" column_name [ "ASC" | "DESC" ] { "," column_name [ "ASC" | "DESC" ] };

condition ::= expression comparison_operator expression | condition logical_operator condition | "(" condition ")";

insert_statement ::= "INSERT INTO" table_name "(" column_name { "," column_name } ")" "VALUES" "(" value { "," value } ")";

update_statement ::= "UPDATE" table_name "SET" column_name "=" value { "," column_name "=" value } [ where_clause ];

delete_statement ::= "DELETE FROM" table_name [ where_clause ];

create_statement ::= "CREATE" object_type object_name [ "AS" select_statement ];

drop_statement ::= "DROP" object_type object_name;

alter_statement ::= "ALTER" object_type object_name modification;

transaction_statement ::= "BEGIN" | "COMMIT" | "ROLLBACK";

object_type ::= "TABLE" | "VIEW" | "INDEX" | "SCHEMA";

modification ::= "ADD" column_definition | "DROP" column_name;

column_definition ::= column_name data_type [ "NOT NULL" ] [ "DEFAULT" value ];

data_type ::= "INTEGER" | "VARCHAR" | "BOOLEAN" | "DATE" | "FLOAT";

value ::= string_literal | numeric_literal | boolean_literal | date_literal;

string_literal ::= "'" { character } "'";
numeric_literal ::= digit { digit };
boolean_literal ::= "TRUE" | "FALSE";
date_literal ::= "'" date_string "'";

date_string ::= digit { digit } "-" digit { digit } "-" digit { digit };

column_name ::= identifier;
table_name ::= identifier;
object_name ::= identifier;
alias ::= "AS" identifier;
identifier ::= #'[A-Za-z][0-9A-Za-z_]*';
comparison_operator ::= "=" | "!=" | "<" | ">" | "<=" | ">=";
logical_operator ::= "AND" | "OR" | "NOT";
letter ::= #'[A-Za-z]';

expression ::= term { ( "+" | "-" ) term };
term ::= factor { ( "*" | "/" ) factor };
factor ::= column_name | value | "(" expression ")";

character ::= letter | digit | special_character;
special_character ::= " " | "!" | "#" | "$" | "%" | "&" | "'" | "(" | ")" | "*" | "+" | "," | "-" | "." | "/" | ":" | ";" | "<" | "=" | ">" | "?" | "@" | "[" | "\\" | "]" | "^" | "_" | "{" | "|" | "}" | "~";
digit ::= #'[0-9]';
    "##;
    // Repeat nodes:
    // identifier ::= letter { letter | digit | "_" };
    do_stuff(sql);
    if let Ok(root) = frontend::create_graph_from_ebnf(sql) {
        root.borrow().dbg();
        let mut cursor = TreeCursor::new(&root);

        let terminal = Term::stdout();
        while !cursor.is_done() {
            let input = terminal.read_char().unwrap();
            match input {
                '\x08' => cursor.clear_inputbuf(),
                _ => {
                    if let Some(res) = cursor.advance(input) {
                        print!("{} ", res);
                    }
                }
            }
            if cursor.is_in_userdefined_stage() {
                print!("{input}");
            }
            std::io::stdout().flush();
        }
    } else {
        eprintln!("Error while creating graph");
    }
    return;
    let root = TreeNode::new_null(None);
    let mut sign_token = NodeType::Keyword(Keyword::new("unsigned".to_string(), None));
    let child = TreeNode::new(sign_token.clone(), &root);
    sign_token = NodeType::Keyword(Keyword::new("signed".to_string(), None));

    let signed = TreeNode::new(sign_token, &root);
    let types = TreeNode::new_required(NodeType::Null, &child);

    let int = TreeNode::new_keyword_with_parent("int".to_string(), types.clone());
    let short = TreeNode::new_keyword_with_parent("short".to_string(), types.clone());
    let short2 = TreeNode::new_keyword_with_parent("shark".to_string(), types.clone());

    let userdefined_node = TreeNode::new_required(
        NodeType::UserDefinedRegex(Regex::new("[0-9]{3,3}").unwrap()),
        &int,
    );
    // let userdefined_node = TreeNode::new_required(
    //     NodeType::UserDefined {
    //         final_chars: vec!['='],
    //     },
    //     &int,
    // );
    let null = TreeNode::new_required(NodeType::Null, &userdefined_node);
    short.borrow_mut().add_child(&userdefined_node);

    signed.borrow_mut().add_child(&types);
    root.borrow_mut().add_child(&types);

    let expression = TreeNode::new(
        NodeType::Keyword(Keyword::new("(".to_string(), Some(")".to_string()))),
        &root,
    );
    let expr_boolvar = TreeNode::new(
        NodeType::UserDefined {
            final_chars: vec![')', '&', '('],
        },
        &expression,
    );
    expr_boolvar.borrow_mut().add_child(&null.clone());
    let cond_and = TreeNode::new(
        NodeType::Keyword(Keyword::new("&&".to_string(), None)),
        &expression,
    );
    cond_and.borrow_mut().add_child(&expr_boolvar);
    expr_boolvar.borrow_mut().add_child(&cond_and);
    expr_boolvar.borrow_mut().add_child(&expression);

    println!("Dump:");
    int.borrow().dump_children();
    // root.borrow().dbg();
    let mut cursor = TreeCursor::new(&root);

    let terminal = Term::stdout();
    while !cursor.is_done() {
        let input = terminal.read_char().unwrap();
        match input {
            '\x08' => cursor.clear_inputbuf(),
            _ => {
                if let Some(res) = cursor.advance(input) {
                    print!("{} ", res);
                }
            }
        }
        if cursor.is_in_userdefined_stage() {
            print!("{input}");
        }
        std::io::stdout().flush();
    }
}
