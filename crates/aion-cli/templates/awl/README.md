# {{name}}

This project starts with one AWL workflow: [`{{name}}.awl`]({{name}}.awl). It has typed input and output, three ordered steps, and a `when`/`otherwise` routing decision. It does not need an activity worker.

## Check and format

Check the document before deploying it:

```sh
aion awl check {{name}}.awl
```

Rewrite it with AWL's canonical formatter after editing:

```sh
aion awl fmt {{name}}.awl
```

## Deploy and run

Start the development server from this directory:

```sh
aion server --config aion.toml
```

In another terminal, deploy the document:

```sh
aion deploy {{name}}.awl
```

Then compile, deploy, start, and await a run in one command:

```sh
aion run {{name}}.awl --input '{"name":"Ada","enthusiastic":true}'
```

The run completes with the `greeted` outcome and the payload `{"message":"Hello, Ada!"}`. Change `enthusiastic` to `false` to take the `otherwise` route.

## Learn the language

Continue with [AWL — the Aion workflow language](https://github.com/ablative-io/aion/blob/main/docs/authoring/AWL.md), the language reference. The interactive authoring tutorial ships in the repository as `docs/authoring/first-workflow.html` — clone the repository and open it locally in a browser (a hosted code view renders it as source, not as slides).
