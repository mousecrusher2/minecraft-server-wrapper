from time import sleep

print("server started")

while True:
    s = input()
    if s == "save hold":
        print("Saving...")
    elif s == "save query":
        print("Data saved. Files are now ready to be copied.")
        print("unko/db/009266.ldb:8822, unko/db/009249.ldb:2132001")
    elif s == "save resume":
        print("Changes to the world are resumed.")
    else:
        print(f"run command: {s}")
