// Test typeof for different value types

// Test 1: Sync object
const syncObj = { foo: "bar" };
console.log("syncObj typeof:", typeof syncObj);
console.log("syncObj.foo:", syncObj.foo);

// Test 2: Sync array
const syncArr = [1, 2, 3];
console.log("syncArr typeof:", typeof syncArr);
console.log("syncArr[0]:", syncArr[0]);

// Test 3: Array of objects
const objArr = [{ id: 1 }, { id: 2 }];
console.log("objArr typeof:", typeof objArr);
console.log("objArr[0] typeof:", typeof objArr[0]);
console.log("objArr[0].id:", objArr[0].id);

// Test 4: Async return
async function getObj(): Promise<{ foo: string }> {
    return { foo: "async" };
}

async function getArr(): Promise<number[]> {
    return [10, 20, 30];
}

async function getObjArr(): Promise<Array<{ id: number }>> {
    return [{ id: 100 }, { id: 200 }];
}

async function main() {
    // Test async object
    const asyncObj = await getObj();
    console.log("asyncObj typeof:", typeof asyncObj);
    console.log("asyncObj.foo:", asyncObj.foo);

    // Test async array
    const asyncArr = await getArr();
    console.log("asyncArr typeof:", typeof asyncArr);
    console.log("asyncArr[0]:", asyncArr[0]);

    // Test async array of objects
    const asyncObjArr = await getObjArr();
    console.log("asyncObjArr typeof:", typeof asyncObjArr);
    console.log("asyncObjArr[0] typeof:", typeof asyncObjArr[0]);
    console.log("asyncObjArr[0].id:", asyncObjArr[0].id);
}

main().catch(console.error);
