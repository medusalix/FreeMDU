# rustcargo-environment in a Docker container

Usage:

`bash rustcargo-environment.sh`

The script pulls a rust-cargo development environment from Docker Hub and starts it. The development directory will be the directory from which the command was initiated; for the FreeMDU project, this will be the FreeMDU folder.

`I have no name!@522db9d80b2e:/usr/src/myapp$ `

Once you get the bash prompt, you can proceed towards home, protocol, or tui:

`cd home`

`cd protocol`

`cd tui`

Program options:

* Delete the container along with all changes (installations): -o delete
If there is no longer any need for the specific container, it shouldn't take up space unnecessarily on the computer's storage device. This does not delete the contents of the working directory, only the running container. Modifications made to the source code remain preserved. It is worth trying this out before starting to make deep modifications, to see what the end result is, and then perhaps it won't cause surprises later.

`bash rustcargo-environment.sh -o delete`

* Delete the container along with all changes (installations) and create a new one: -o new
As described in the previous point, all previous changes in the container are deleted and the container itself is destroyed. A new one is created. If settings need to be modified, it is worth using this. When we start a container, the settings take effect then. After that, they cannot be changed—only deleting and creating the container with new settings is possible. Modifications made to the source code remain preserved.

`bash rustcargo-environment.sh -o new`

The container settings can be found here in the script:

```
docker run -dit \
    --name "$CONTAINER_NAME" \
    --user "$(id -u):$(id -g)" \
    --group-add $(getent group dialout | cut -d: -f3) \
    -v "$PWD":"$WORKDIR" \
    -w "$WORKDIR" \
    --device=/dev/ttyACM0 \
    "$IMAGE" \
    bash >/dev/null
```

As shown by the settings, the current user must have **dialout** group membership.

In my experience, the **ESP32-c3** appeared at **/dev/ttyACM0**. I have not tested it with the ESP32-c6.

You can read about further options in those folders:

[home/README](home/README.md)

[protocol/README](protocol/README.md)

[tui/README](tui/README.md)



# rustcargo-environment egy docker konténerben

Használat:

`bash rustcargo-environment.sh`

A szkript lehúz a docker-hub-ról egy rust-cargo fejlesztői környezetet és el is indítja. A fejlesztői könyvtár az a könyvtár lesz, amelyikből a parancsot indítottuk, ez a FreeMDU projekt esetén a FreeMDU mappa lesz. 

`I have no name!@522db9d80b2e:/usr/src/myapp$`

Ha megkaptuk a bash promptot, akkor mehetünk tovább a home, a protocol vagy  a tui irányába:

`cd home`

`cd protocol`

`cd tui`

A program opciói:

* Törölni a konténert az összes változtatással (telepítéssel) együtt: -o delete  
Ha már semmi szükség nincs az adott konténerre, ne foglalja feleslegesen a helyet a számítógép adathordozóján. Ez nem törli a munkakönyvtár tartalmát, csak a futó konténert. A forráskódban végzett módosítások megmaradnak. Érdemes kipróbálni, mielőtt még hozzákezdenénk mélyen a módosításokhoz, hogy mi a végeredménye, s akkor talán később nem okoz meglepetést.

`bash rustcargo-environment.sh -o delete`

* Törölni a konténert az összes változtatással (telepítéssel) együtt és egy újat létrehozni: -o new  
Az előző pont szerint törlődik minden korábbi változtatás a konténerből és a konténer is megsemmisül. Egy új jön létre. Ha változtatni szükséges a beállításokat, akkor érdemes ezt használni. Amikor elindítunk egy konténert, a beállítások ott jutnak érvényre. Utána már nem lehet változtatni, csak törölni és új beállításokkal létrehozni a konténert. A forráskódban végzett módosítások megmaradnak.

`bash rustcargo-environment.sh -o new`

A konténer beállításai itt találhatóak a szkriptben:


```
docker run -dit \
    --name "$CONTAINER_NAME" \
    --user "$(id -u):$(id -g)" \
    --group-add $(getent group dialout | cut -d: -f3) \
    -v "$PWD":"$WORKDIR" \
    -w "$WORKDIR" \
    --device=/dev/ttyACM0 \
    "$IMAGE" \
    bash >/dev/null
```
    
A beállításokból kitűnik, hogy az aktuális felhasználónak rendelkezni kell a **dialout** csoporttagsággal. 

A tapasztalatom szerint az ESP32-c3 a **/dev/ttyACM0** helyen jelentkezett. Az ESP32-c6-tal nem teszteltem. 

A további lehetőségekről azokban a mappákban lehet olvasni:

[home/README.md](home/README.md)  

[protocol/README.md](protocol/README.md)  

[tui/README.md](tui/README.md)  

