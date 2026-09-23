package tree_sitter_mprx_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_mprx "github.com/ValanceX/Mesh/grammar/tree-sitter-mprx/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_mprx.Language())
	if language == nil {
		t.Errorf("Error loading Mprx grammar")
	}
}
