#include "tree_sitter/parser.h"
#include <wctype.h>

// Nested block comments are the only token that needs an external scanner.
// All state is local to scan(), so incremental parses need no serialized state.
enum TokenType { BLOCK_COMMENT };

void *tree_sitter_lygos_external_scanner_create(void) { return NULL; }
void tree_sitter_lygos_external_scanner_destroy(void *payload) { (void)payload; }
unsigned tree_sitter_lygos_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload; (void)buffer; return 0;
}
void tree_sitter_lygos_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  (void)payload; (void)buffer; (void)length;
}

bool tree_sitter_lygos_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  (void)payload;
  if (!valid_symbols[BLOCK_COMMENT]) return false;
  while (iswspace(lexer->lookahead)) lexer->advance(lexer, true);
  if (lexer->lookahead != '/') return false;
  lexer->advance(lexer, false);
  if (lexer->lookahead != '*') return false;
  lexer->advance(lexer, false);
  unsigned depth = 1;
  while (depth) {
    if (lexer->eof(lexer)) return false;
    int32_t c = lexer->lookahead;
    lexer->advance(lexer, false);
    if (c == '/' && lexer->lookahead == '*') {
      lexer->advance(lexer, false);
      depth++;
    } else if (c == '*' && lexer->lookahead == '/') {
      lexer->advance(lexer, false);
      depth--;
    }
  }
  lexer->mark_end(lexer);
  lexer->result_symbol = BLOCK_COMMENT;
  return true;
}
