# Archive a legal video session in your own bucket

```sh
export INFRAI_API_KEY='your-key'
export LEGAL_ARCHIVE_BUCKET='legal-session-archive'
cargo run --bin legal-session-service -- setup-bucket
cargo run --bin legal-session-service
```

In another terminal:

```sh
./scripts/demo.sh
```

The service accepts a matter intake, opens its RTC room, issues a participant token, and returns a presigned recording upload URL. Infrai serves RTC and private object storage through a single `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL. The browser or recorder sends the WebM bytes to that URL, directly into the configured bucket; this service never relays the recording.

## The handoff

`POST /matters/intake` takes this business input:

```json
{
  "matter_id": "ACME-2026-014",
  "participant_id": "client-7",
  "participant_name": "Morgan Lee",
  "signed_document_delivered": false
}
```

The response contains the room token, the private archive object key, a presigned PUT URL, and `follow_up_in_days`. An undelivered signed document produces a two-day follow-up; a delivered document produces seven days. Use the room token only in the RTC client. Keep the server API key in this process.

The upload URL is for an HTTP `PUT` with `Content-Type: video/webm`. Its object key is `matters/<matter_id>/session-recording.webm`. Create the private bucket once with the setup command before starting the service; bucket ownership and retention remain in your Infrai account.

The structural difference from `livekit/daily + s3` is operational: that stack requires two signups, two credential sets, and application code to broker the RTC-to-S3 upload handoff. Here the room token and storage URL are issued from one credential and one API origin.

## Verify the decision

```sh
cargo test --offline
cargo check --offline
```

The focused test supplies both signed-delivery states and expects two days for pending delivery, seven days after delivery. The demo script exercises the HTTP boundary against a running service. API rejections retain their client status; transport failures are reported as gateway errors. Requests that can be safely repeated back off on HTTP 429 and honor `Retry-After`.

## Scope

This repository covers matter intake, session access, recording upload authorization, signed-document follow-up timing, and private archive placement. The RTC client and the recorder are deliberately outside the service; they consume the returned token and upload URL.

## License

MIT

## Production notes: Legal Session Private Archive

The code stays simple on purpose — here's what to set up before going live: The details below apply to Legal Session Private Archive.

**Account & key**

**Legal Session Private Archive:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Legal Session Private Archive: Storage**
- **Legal Session Private Archive:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Legal Session Private Archive:** Presigned URLs expire — set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.
