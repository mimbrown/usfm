// @ts-check

// Script run within the webview itself.
(function () {
  // Get a reference to the VS Code webview api.
  // We use this API to post messages back to our extension.

  // @ts-ignore
  const vscode = acquireVsCodeApi();

  const arrow = /\s*>\s*/;

  const mappingsContainer = /** @type {HTMLElement} */ (
    document.querySelector(".mappings")
  );

  // const addButtonContainer = document.querySelector(".add-button");
  // addButtonContainer.querySelector("button").addEventListener("click", () => {
  //   vscode.postMessage({
  //     type: "add",
  //   });
  // });

  const errorContainer = document.createElement("div");
  document.body.appendChild(errorContainer);
  errorContainer.className = "error";
  errorContainer.style.display = "none";

  /**
   * Render the document in the webview.
   */
  function updateContent(/** @type {string} */ text) {
    console.log("Got", text);
    mappingsContainer.innerHTML = "";
    for (let [index, line] of text.split("\n").sort().entries()) {
      line = line.trim();
      if (line) {
        const [key, value = ""] = line.split(arrow);
        const dt = document.createElement("dt");
        const dd = document.createElement("dd");
        dt.textContent = key;
        dd.textContent = value;
        dt.dataset.line = index.toString();
        dd.dataset.line = index.toString();
        mappingsContainer.appendChild(dt);
        mappingsContainer.appendChild(dd);
      }
    }
  }

  // Handle messages sent from the extension to the webview
  window.addEventListener("message", (event) => {
    const message = event.data; // The json data that the extension sent
    switch (message.type) {
      case "update":
        const text = message.text;

        // Update our webview's content
        updateContent(text);

        // Then persist state information.
        // This state is returned in the call to `vscode.getState` below when a webview is reloaded.
        vscode.setState({ text });

        return;
    }
  });

  document.body.addEventListener("click", (event) => {
    /** @type {HTMLElement | null} */
    const target = /** @type {HTMLElement} */ (event.target).closest(
      "[data-line]"
    );
    if (target) {
      const content = target.textContent;
      const popover = document.createElement("div");
      popover.popover = "auto";
      popover.className = "editing";
      target.classList.add("editing-target");

      popover.innerHTML = `<form><input name="text" type="text" value="${content}" /></form>`;
      const form = /** @type {HTMLFormElement} */ (
        popover.querySelector("form")
      );
      const input = /** @type {HTMLInputElement} */ (
        popover.querySelector("input")
      );

      form.onsubmit = (event) => {
        event.preventDefault();
        popover.hidePopover();
        const { value: text } = input;
        let key, value;
        if (target.tagName === "DT") {
          key = text;
          value = /** @type {HTMLElement} */ (target.nextElementSibling)
            .textContent;
        } else {
          key = /** @type {HTMLElement} */ (target.previousElementSibling)
            .textContent;
          value = text;
        }
        vscode.postMessage({
          type: "update",
          key,
          value,
          line: Number(target.dataset.line),
        });
      };

      popover.addEventListener("beforetoggle", (event) => {
        if (event.newState === "closed") {
          target.classList.remove("editing-target");
          popover.remove();
        }
      });

      document.body.appendChild(popover);
      popover.showPopover();
      input.focus();
      input.select();
    }
  });

  // Webviews are normally torn down when not visible and re-created when they become visible again.
  // State lets us save information across these re-loads
  const state = vscode.getState();
  if (state) {
    updateContent(state.text);
  }
})();
