package export_wit_world

import (
	"fmt"
	"runtime"
	"runtime/debug"
	"time"
	"wit_component/wit_world"
)

func Run() {
	timer := time.AfterFunc(time.Hour, func() { panic("timer fired early") })
	defer timer.Stop()
	old := debug.SetGCPercent(10)
	defer debug.SetGCPercent(old)
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	var retained [8]string
	for i := range 4096 {
		retained[i%len(retained)] = wit_world.GetStr()
		if len(retained[i%len(retained)]) != 65536 {
			panic("wrong import result length")
		}
	}
	runtime.KeepAlive(retained)
	runtime.ReadMemStats(&after)
	if after.NumGC == before.NumGC {
		panic("test did not trigger garbage collection")
	}
	fmt.Println("ok")
}
