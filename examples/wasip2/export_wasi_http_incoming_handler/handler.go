package export_wasi_http_incoming_handler

import (
	. "wit_component/wasi_http_types"

	. "go.bytecodealliance.org/pkg/wit/types"
)

// Handle the specified `Request`, returning a `Response`
func Handle(request *IncomingRequest, responseOut *ResponseOutparam) {
	response := MakeOutgoingResponse(MakeFields())

	body := response.Body()

	ResponseOutparamSet(responseOut, Ok[*OutgoingResponse, ErrorCode](response))

	message := []byte("Hello, world!")

	if body.IsOk() {
		stream := body.Ok()
		if writeResult := stream.Write(); writeResult.IsOk() {
			writeResult.Ok().BlockingWriteAndFlush(message)
		}
	}
}
