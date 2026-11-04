# Opening files from the file manager

glowfish can be added to the right-click menu of your file manager, so measurement files and
folders open in the viewer without typing a command. This page sets that up for the `glowfish`
command installed as described in [Installation](../installation.md).

QASS measurement files have no common file extension: names like `p000665c0b01` or
`p000665c0b01.000` are typical. What they share are their first 8 bytes, `qassdata`. Linux can
recognize files by these bytes; Windows and macOS only go by the file extension, so there
glowfish is offered for all files instead.

=== "Linux"

    Linux desktops (GNOME, KDE, Xfce and others) recognize files by their content, so glowfish
    can be offered for measurement files only, and even become their default application.

    **1. Define a file type** for files that start with `qassdata`:

    ```sh
    mkdir -p ~/.local/share/mime/packages
    cat > ~/.local/share/mime/packages/qass.xml <<'EOF'
    <?xml version="1.0" encoding="UTF-8"?>
    <mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
      <mime-type type="application/x-qass">
        <comment>QASS measurement file</comment>
        <magic priority="80">
          <match type="string" offset="0" value="qassdata"/>
        </magic>
      </mime-type>
    </mime-info>
    EOF
    update-mime-database ~/.local/share/mime
    ```

    **2. Register glowfish** as an application for that type:

    ```sh
    mkdir -p ~/.local/share/applications
    cat > ~/.local/share/applications/glowfish.desktop <<EOF
    [Desktop Entry]
    Type=Application
    Name=glowfish
    Comment=View QASS measurement files
    Exec=$HOME/.cargo/bin/glowfish %f
    Terminal=false
    MimeType=application/x-qass;
    Categories=Science;Viewer;
    EOF
    update-desktop-database ~/.local/share/applications
    ```

    `$HOME` is filled in when the file is written: desktop entries do not expand it
    themselves. If glowfish is installed somewhere else, change the `Exec` line.

    glowfish now appears under *Open With* for measurement files.

    **3. Make glowfish the default** for measurement files, so a double click opens them
    (optional):

    ```sh
    xdg-mime default glowfish.desktop application/x-qass
    ```

    !!! tip "Opening folders"
        To offer glowfish for folders as well, change the `MimeType` line to
        `MimeType=application/x-qass;inode/directory;` and run `update-desktop-database`
        again. glowfish then appears under *Open With* for folders, without becoming the
        default for them.

    **To remove** the setup again:

    ```sh
    rm ~/.local/share/mime/packages/qass.xml ~/.local/share/applications/glowfish.desktop
    update-mime-database ~/.local/share/mime
    update-desktop-database ~/.local/share/applications
    ```

=== "Windows"

    Windows decides by file extension only, so glowfish is added to the right-click menu of
    all files as **Open with glowfish**.

    Run these commands in a **Command Prompt** (`cmd`, not PowerShell, which treats `%` and
    quotes differently). They only change your user account and need no administrator
    rights:

    ```bat
    reg add "HKCU\Software\Classes\*\shell\glowfish" /ve /d "Open with glowfish" /f
    reg add "HKCU\Software\Classes\*\shell\glowfish" /v Icon /d "%USERPROFILE%\.cargo\bin\glowfish.exe" /f
    reg add "HKCU\Software\Classes\*\shell\glowfish\command" /ve /d "\"%USERPROFILE%\.cargo\bin\glowfish.exe\" \"%1\"" /f
    ```

    `%USERPROFILE%` is replaced by your user folder when the commands run; `%1` stays as it
    is and stands for the clicked file. If glowfish is installed somewhere else, change the
    path.

    On Windows 11, the entry is under *Show more options* in the right-click menu (or
    ++shift+f10++).

    !!! tip "Opening folders"
        Run the same three commands with `Directory` in place of `*`, e.g.
        `HKCU\Software\Classes\Directory\shell\glowfish`, to add the entry to folders as well.

    !!! note "The console window"
        glowfish is a command line program, so Windows opens a console window next to the
        viewer. Closing the console window closes glowfish as well.

    **To remove** the entry again:

    ```bat
    reg delete "HKCU\Software\Classes\*\shell\glowfish" /f
    reg delete "HKCU\Software\Classes\Directory\shell\glowfish" /f
    ```

=== "macOS"

    macOS decides by file extension as well, and only lists app bundles under *Open With*,
    not command line programs. A **Quick Action** adds glowfish to the right-click menu of all
    files and folders instead.

    1. Open **Automator** (in *Applications*) and create a new document of type
       **Quick Action**.
    2. At the top, set *Workflow receives current* to **files or folders** in **Finder**.
    3. Search for the action **Run Shell Script** on the left and drag it into the workflow.
       Set *Pass input* to **as arguments** and replace the script with:

        ```sh
        for f in "$@"; do
          "$HOME/.cargo/bin/glowfish" "$f" > /dev/null 2>&1 &
        done
        ```

        Each selected file opens in its own window. If glowfish is installed somewhere else,
        change the path.

    4. Save the Quick Action as **Open with glowfish**.

    Right-click a file or folder in Finder and choose **Quick Actions → Open with glowfish**.
    The first time, macOS may ask whether glowfish may access the folder.

    **To remove** the Quick Action, delete `~/Library/Services/Open with glowfish.workflow`.
