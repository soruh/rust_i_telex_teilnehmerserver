logged_in((err, is_logged_in) => {
  if (typeof main === "function") {
    console.log("logged in", is_logged_in);
    main(is_logged_in);
  } else {
    alert("no main function found");
  }
});
