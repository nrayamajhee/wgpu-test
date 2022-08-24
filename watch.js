setInterval(async () => {
  const res = await fetch("http://localhost:8080/check-state")
  const data = await res.text()
  if (data == "stale") {
      await fetch("http://localhost:8080/reset-state")
      location.reload()
  }
}, 2000)
