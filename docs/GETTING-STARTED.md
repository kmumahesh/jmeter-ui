# Getting started

JMeter UI is a desktop app for building and running Apache JMeter test plans. It replaces JMeter's own window with a faster, clearer one, and runs your plans with the real JMeter, so results are the ones JMeter would give.

This page takes you from the download to your first test run and a look at what happened. It takes about ten minutes.

> **This is a test version.** It is built for performance engineers to try on Windows, for HTTP and HTTPS web and API testing. Things will change. Please tell us what gets in your way: [open an issue](https://github.com/kmumahesh/jmeter-ui/issues).

## What you need

| You need | Why | Where to get it |
|---|---|---|
| Windows 10 or 11 (64-bit) | The app is built and tested on Windows first | |
| Java 17 or newer | JMeter is a Java program | [Eclipse Temurin](https://adoptium.net/) (choose the JDK or JRE, 64-bit) |
| Apache JMeter 5.6.3 | The app runs your plans with it | [jmeter.apache.org](https://jmeter.apache.org/download_jmeter.cgi) (the binary `.zip`) |

You do not need to start JMeter yourself or set up any environment variable.

## Install

1. Download `jmeter-ui-0.1.0-windows-x64.zip` from the [releases page](https://github.com/kmumahesh/jmeter-ui/releases).
2. Check the download (optional, recommended): compare the SHA-256 in `SHA256SUMS.txt` with `Get-FileHash jmeter-ui-0.1.0-windows-x64.zip` in PowerShell.
3. Unzip it anywhere, for example `C:\Tools\jmeter-ui`.
4. Unzip the JMeter download too. A folder called `apache-jmeter-5.6.3` in your *Downloads* folder is found by itself; any other place you choose in Settings.
5. Start `jmeter-ui.exe`.

> **Windows may warn about an unknown publisher** the first time (SmartScreen). The app is not signed with a certificate yet. Choose *More info*, then *Run anyway*, if you trust the download.

## The first start

The welcome screen checks for Java and JMeter and tells you what it found:

- **Ready to run: Java 21 · JMeter 5.6.3** means you are set.
- **Java was not found** or **JMeter was not found** shows what to do. Install it, or press *Open Settings* and choose the JMeter folder (the one that contains `bin`) and, if needed, `java.exe`. Press *Check again*.

## Make and run your first test

1. Press **Ctrl+N** and choose **API test**. Save it where you like.
2. In the tree on the left, select **HTTP Request Defaults** and type the server you want to test (for example `api.example.com`) in **Server name or IP**. Change **Protocol** to `http` if it is not HTTPS.
3. Select **Get items** and set **Path** to a real path on that server.
4. Press **F5**. The lower panel shows the run: users, requests per second, average and 95th percentile response time, errors, and live charts.
5. When it finishes, the run is kept in the **History** tab, in a `runs` folder next to your plan.

> **Tip:** press **Ctrl+F5** to check the plan first with one user and no pauses, or **F6** to send just the selected request once and see what came back.

## Look at what happened

| To see | Do this |
|---|---|
| What was sent and what came back for a request, including failed ones | **Ctrl+Shift+R** (the *Requests* tab). By default only failed requests are kept; choose *All requests* in Settings to keep every request. |
| Numbers for each request: median, percentiles, standard deviation, rate | The *Requests* table under the run charts. Click a request to chart just that one. *Copy table as CSV* puts it on the clipboard. |
| Whether something got slower | In *History*, press **Compare** on the baseline run and then on the run to check. |
| A report to share | **Ctrl+Shift+H** makes JMeter's HTML report and opens it in your browser. |
| A results file from somewhere else | **File > Open Results File** (Ctrl+Shift+O), a `.jtl` or `.csv`. |

## Build a plan from what you already have

- **From a browser:** in your browser's developer tools (Network tab) use *Save all as HAR*, then **File > Import from HAR File**. Each page becomes a Transaction Controller. Images, styles and scripts are left out unless you ask.
- **From a single request:** copy it with *Copy as cURL* and press **Ctrl+Shift+I**.
- **From a template:** **Ctrl+N** also offers a login-and-token test, a data-driven test, a load test with a time limit and a spike test.
- **Add pauses:** select a thread group or controller and use **Edit > Add Think Times to Children**.
- **Test a value you pull out of a response:** **Ctrl+Shift+T** tries a JSON Path, regular expression, XPath or CSS selector on a response and shows what JMeter would take.

## Where your files go

- Your plans are `.jmx` files, the same format JMeter uses, so they open in JMeter too.
- Each run gets its own folder under `runs` next to the plan (or in a folder you choose in Settings) with its results, log, the plan as it ran and, if you make one, the report.
- Settings are in `%APPDATA%\jmeter-ui\settings.json`.
- The app sends nothing anywhere.

## If something goes wrong

| What you see | What to do |
|---|---|
| *Java was not found* | Install Java 17 or newer, restart the app, or choose `java.exe` in Settings. |
| *JMeter was not found* | Choose the folder that contains `bin\ApacheJMeter.jar` in Settings. |
| The run fails at once | Look at the run header and the *JMeter output* tab. A path with letters Windows cannot pass to Java (some non-Latin characters) can stop JMeter finding the plan: move it to a plainer folder. |
| A plan says it *can run code* | It has scripts in it. Read the question; run it only if you trust where the plan came from. |
| The window is slow with a very big plan | Plans up to about 60,000 elements open in under five seconds. Bigger plans open, more slowly. |
| Anything else | [Open an issue](https://github.com/kmumahesh/jmeter-ui/issues) with what you did and what you expected. Include the file `jmeter.log` from the run folder if a run is involved, after checking it holds nothing private. |

## Keyboard shortcuts

Press **Ctrl+K** for every command with its shortcut. The ones you will use most:

| Key | What it does |
|---|---|
| F5 / Shift+F5 | Run / stop |
| Ctrl+F5 | Check the plan with one user |
| F6 | Send the selected request once |
| Ctrl+Shift+R | Requests and responses of the run |
| Ctrl+Shift+T | Try an expression on a response |
| Ctrl+Shift+H | HTML report |
| Ctrl+Shift+A | Add an element |
| Ctrl+P | Go to an element by name |
| Ctrl+Z / Ctrl+Y | Undo / redo |
