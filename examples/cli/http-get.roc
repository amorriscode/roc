app [main] { pf: platform "https://github.com/roc-lang/basic-cli/releases/download/0.8.1/x8URkvfyi9I0QhmVG98roKBUs_AZRkLFwFJVJ3942YA.tar.br" }

import pf.Http
import pf.Task exposing [Task]
import pf.Stdin
import pf.Stdout

main : Task {} I32
main =
    _ <- Task.await (Stdout.line "Enter a URL to fetch. It must contain a scheme like \"http://\" or \"https://\".")

    input <- Task.await Stdin.line

    when input is
        End ->
            Stdout.line "I received end-of-input (EOF) instead of a URL."

        Input url ->
            request = {
                method: Get,
                headers: [],
                url,
                body: Http.emptyBody,
                timeout: NoTimeout,
            }

            output <- Http.send request
                |> Task.onErr \err -> err
                    |> Http.errorToString
                    |> Task.ok
                |> Task.await

            Stdout.line output
