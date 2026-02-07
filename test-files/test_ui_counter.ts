import { App, VStack, Text, Button, State } from "perry/ui"

const count = State(0)

App({
    title: "Counter",
    width: 400,
    height: 300,
    body: VStack(16, [
        Text(`Count: ${count.value}`),
        Button("Increment", () => count.set(count.value + 1)),
    ])
})
