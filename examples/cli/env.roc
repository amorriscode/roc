app [main] { pf: platform "https://github.com/roc-lang/basic-cli/releases/download/0.8.1/x8URkvfyi9I0QhmVG98roKBUs_AZRkLFwFJVJ3942YA.tar.br" }

import pf.Stdout
import pf.Stderr
import pf.Env
import pf.Task exposing [Task]

main : Task {} I32
main =
    task =
        Env.decode "EDITOR"
        |> Task.await (\editor -> Stdout.line "Your favorite editor is $(editor)!")
        |> Task.await (\{} -> Env.decode "SHLVL")
        |> Task.await
            (\lvl ->
                when lvl is
                    1u8 -> Stdout.line "You're running this in a root shell!"
                    n ->
                        lvlStr = Num.toStr n

                        Stdout.line "Your current shell level is $(lvlStr)!")
        |> Task.await \{} -> Env.decode "LETTERS"

    Task.attempt task \result ->
        when result is
            Ok letters ->
                joinedLetters = Str.joinWith letters " "

                Stdout.line "Your favorite letters are: $(joinedLetters)"

            Err _ ->
                Stderr.line "I couldn't find your favorite letters in the environment variables!"
