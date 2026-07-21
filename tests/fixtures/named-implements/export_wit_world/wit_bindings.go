package export_wit_world

import (
	"fmt"

	"wit_component/foo_baz_store"
	"wit_component/primary"
	"wit_component/secondary"
)

// Run calls the same `store.get` function through the two named imports and
// the bare import, proving each routes to its own host instance.
func Run() {
	fmt.Println(primary.Get("color"))
	fmt.Println(secondary.Get("color"))
	fmt.Println(foo_baz_store.Get("color"))
}
