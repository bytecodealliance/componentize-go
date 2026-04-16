package main

import (
	"archive/tar"
	"compress/gzip"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/apparentlymart/go-userdirs/userdirs"
	"github.com/gofrs/flock"
)

// This is a simple wrapper program which downloads, caches, and runs the
// appropriate `componentize-go` binary for the current platform.
//
// Although `componentize-go` is written in Rust, we can use this wrapper to
// make it available using e.g. `go install` and/or `go tool`.
func main() {
	release := "v0.3.2"

	directories := userdirs.ForApp(
		"componentize-go",
		"bytecodealliance",
		"com.github.bytecodealliance-componentize-go",
	)
	binDirectory := filepath.Join(directories.CacheDir, "bin")
	versionPath := filepath.Join(directories.CacheDir, "version.txt")
	lockFilePath := filepath.Join(directories.CacheDir, "lock")
	binaryPath := filepath.Join(binDirectory, "componentize-go")

	maybeDownload(release, binDirectory, versionPath, lockFilePath, binaryPath)

	run(binaryPath)
}

// Download the specified version of `componentize-go` if we haven't already.
func maybeDownload(release, binDirectory, versionPath, lockFilePath, binaryPath string) {
	if err := os.MkdirAll(binDirectory, 0755); err != nil {
		log.Fatalf("unable to create directory `%v`: %v", binDirectory, err)
	}

	// Lock the lock file to prevent concurrent downloads.
	lockFile := flock.New(lockFilePath)
	if err := lockFile.Lock(); err != nil {
		log.Fatalf("unable to lock file `%v`: %v", lockFilePath, err)
	}
	defer lockFile.Unlock()

	versionBytes, err := os.ReadFile(versionPath)
	var version string
	if err != nil {
		version = "<unknown>"
	} else {
		version = strings.TrimSpace(string(versionBytes))
	}

	// If the binary doesn't already exist and/or the version doesn't match
	// the desired release, download it.
	if _, err := os.Stat(binaryPath); errors.Is(err, os.ErrNotExist) || version != release {
		base := fmt.Sprintf(
			"https://github.com/bytecodealliance/componentize-go/releases/download/%v",
			release,
		)

		url := fmt.Sprintf("%v/componentize-go-%v-%v.tar.gz", base, runtime.GOOS, runtime.GOARCH)

		fmt.Printf("Downloading `componentize-go` binary from %v and extracting to %v\n", url, binDirectory)

		response, err := http.Get(url)
		if err != nil {
			log.Fatalf("unable to download URL `%v`: %v", url, err)
		}
		defer response.Body.Close()

		if response.StatusCode < 200 || response.StatusCode > 299 {
			log.Fatalf("unexpected status for URL `%v`: %v", url, response.StatusCode)
		}

		uncompressed, err := gzip.NewReader(response.Body)
		if err != nil {
			log.Fatalf("unable to decompress content of URL `%v`: %v", url, err)
		}
		defer uncompressed.Close()

		untarred := tar.NewReader(uncompressed)
		for {
			header, err := untarred.Next()
			if err == io.EOF {
				break
			} else if err != nil {
				log.Fatalf("unable to untar content of URL `%v`: %v", url, err)
			}
			path := filepath.Join(binDirectory, header.Name)
			file, err := os.Create(path)
			if err != nil {
				log.Fatalf("unable to create file `%v`: %v", path, err)
			}
			if _, err := io.Copy(file, untarred); err != nil {
				log.Fatalf("unable to untar content of URL `%v`: %v", url, err)
			}
			file.Close()
		}

		if err := os.Chmod(binaryPath, 0755); err != nil {
			log.Fatalf("unable to make file `%v` executable: %v", binaryPath, err)
		}
	}

	// If we just downloaded a new version, remember which one so we don't
	// download it redundantly next time.
	if version != release {
		if err := os.WriteFile(versionPath, []byte(release), 0600); err != nil {
			log.Fatalf("unable to write version to `%v`: %v", versionPath, err)
		}
	}
}

// Run the specified binary, forwarding all our arguments to it and piping its
// stdout and stderr back to the user.
func run(binaryPath string) {
	command := exec.Command(binaryPath, os.Args[1:]...)
	command.Stdout = os.Stdout
	command.Stderr = os.Stderr

	if err := command.Start(); err != nil {
		log.Fatalf("unable to start `%v` command: %v", binaryPath, err)
	}

	if err := command.Wait(); err != nil {
		if exiterr, ok := err.(*exec.ExitError); ok {
			code := exiterr.ExitCode()
			if code != 0 {
				os.Exit(code)
			}
		} else {
			log.Fatalf("trouble running `%v` command: %v", binaryPath, err)
		}
	}
}
