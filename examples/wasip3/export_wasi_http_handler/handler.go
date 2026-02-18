package export_wasi_http_handler

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/url"
	"wit_component/wasi_http_handler"
	. "wit_component/wasi_http_types"

	. "go.bytecodealliance.org/pkg/wit/types"
)

// Handle the specified `Request`, returning a `Response`
func Handle(request *Request) Result[*Response, ErrorCode] {
	method := request.GetMethod().Tag()
	path := request.GetPathWithQuery().SomeOr("/")

	if method == MethodGet && path == "/hello" {
		// Say hello!

		tx, rx := MakeStreamU8()

		go func() {
			defer tx.Drop()
			tx.WriteAll([]uint8("Hello, world!"))
		}()

		response, send := ResponseNew(
			FieldsFromList([]Tuple2[string, []byte]{
				{F0: "content-type", F1: []byte("text/plain")},
			}).Ok(),
			Some(rx),
			trailersFuture(),
		)
		send.Drop()

		return Ok[*Response, ErrorCode](response)

	} else if method == MethodGet && path == "/hash-all" {
		// Collect one or more "url" headers, download their contents
		// concurrently, compute their SHA-256 hashes incrementally
		// (i.e. without buffering the response bodies), and stream the
		// results back to the client as they become available.

		urls := make([]string, 0)
		for _, pair := range request.GetHeaders().CopyAll() {
			if pair.F0 == "url" {
				urls = append(urls, string(pair.F1))
			}
		}

		tx, rx := MakeStreamU8()

		go func() {
			defer tx.Drop()

			channel := make(chan Tuple2[string, string])
			for _, url := range urls {
				go func() {
					channel <- Tuple2[string, string]{F0: url, F1: getSha256(url)}
				}()
			}

			for i := 0; i < len(urls); i++ {
				pair := (<-channel)
				tx.WriteAll([]uint8(fmt.Sprintf("%v: %v\n", pair.F0, pair.F1)))
			}
		}()

		response, send := ResponseNew(
			FieldsFromList([]Tuple2[string, []uint8]{
				{F0: "content-type", F1: []uint8("text/plain")},
			}).Ok(),
			Some(rx),
			trailersFuture(),
		)
		send.Drop()

		return Ok[*Response, ErrorCode](response)

	} else if method == MethodPost && path == "/echo" {
		// Echo the request body back to the client without buffering.

		requestHeaders := request.GetHeaders().CopyAll()

		rx, trailers := RequestConsumeBody(request, unitFuture())

		responseHeaders := make([]Tuple2[string, []uint8], 0, 1)
		for _, pair := range requestHeaders {
			if pair.F0 == "content-type" {
				responseHeaders = append(responseHeaders, pair)
			}
		}

		response, send := ResponseNew(
			FieldsFromList(responseHeaders).Ok(),
			Some(rx),
			trailers,
		)
		send.Drop()

		return Ok[*Response, ErrorCode](response)

	} else {
		// Bad request

		response, send := ResponseNew(
			MakeFields(),
			None[*StreamReader[uint8]](),
			trailersFuture(),
		)
		send.Drop()
		response.SetStatusCode(400).Ok()

		return Ok[*Response, ErrorCode](response)

	}
}

// Download the contents of the specified URL, computing the SHA-256
// incrementally as the response body arrives.
//
// This returns a tuple of the original URL and either the hex-encoded hash or
// an error message.
func getSha256(urlString string) string {
	parsed, err := url.Parse(urlString)
	if err != nil {
		return err.Error()
	}

	var scheme Scheme
	switch parsed.Scheme {
	case "http":
		scheme = MakeSchemeHttp()
	case "https":
		scheme = MakeSchemeHttps()
	default:
		scheme = MakeSchemeOther(parsed.Scheme)
	}

	request, send := RequestNew(
		MakeFields(),
		None[*StreamReader[uint8]](),
		trailersFuture(),
		None[*RequestOptions](),
	)
	send.Drop()
	request.SetScheme(Some(scheme)).Ok()
	request.SetAuthority(Some(parsed.Host)).Ok()
	request.SetPathWithQuery(Some(parsed.Path)).Ok()

	result := wasi_http_handler.Handle(request)
	switch result.Tag() {
	case ResultOk:
		response := result.Ok()
		status := response.GetStatusCode()
		if status < 200 || status > 299 {
			return fmt.Sprintf("unexpected status: %v", status)
		}

		rx, trailers := ResponseConsumeBody(response, unitFuture())
		trailers.Drop()
		defer rx.Drop()

		buffer := make([]uint8, 16*1024)
		hash := sha256.New()
		for !rx.WriterDropped() {
			count := rx.Read(buffer)
			writeCount, err := hash.Write(buffer[:count])
			if err != nil || uint32(writeCount) != count {
				panic("unreachable")
			}
		}
		return hex.EncodeToString(hash.Sum([]uint8{}))

	case ResultErr:
		return "error sending request"

	default:
		panic("unreachable")
	}
}

func trailersFuture() *FutureReader[Result[Option[*Fields], ErrorCode]] {
	tx, rx := MakeFutureResultOptionFieldsErrorCode()
	go tx.Write(Ok[Option[*Fields], ErrorCode](None[*Fields]()))
	return rx
}

func unitFuture() *FutureReader[Result[Unit, ErrorCode]] {
	tx, rx := MakeFutureResultUnitErrorCode()
	go tx.Write(Ok[Unit, ErrorCode](Unit{}))
	return rx
}
