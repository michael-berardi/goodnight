import XCTest
@testable import UpdateKit

final class UpdateKitTests: XCTestCase {
    func testOnlyStrictlyNewerStableVersionsCount() {
        XCTAssertEqual(ReleaseVersion.parts("0.3.0"), [0, 3, 0])
        for bad in ["v0.3.0", "0.3.0-beta.1", "0.3", "1.2.3.4", "", "newest", "0.3.x"] {
            XCTAssertNil(ReleaseVersion.parts(bad), bad)
        }
        XCTAssertTrue(ReleaseVersion.isNewer("0.4.0", than: "0.3.0"))
        XCTAssertTrue(ReleaseVersion.isNewer("0.3.1", than: "0.3.0"))
        XCTAssertTrue(ReleaseVersion.isNewer("0.10.0", than: "0.9.9"))
        XCTAssertTrue(ReleaseVersion.isNewer("1.0.0", than: "0.99.99"))
        XCTAssertFalse(ReleaseVersion.isNewer("0.3.0", than: "0.3.0"))
        XCTAssertFalse(ReleaseVersion.isNewer("0.2.9", than: "0.3.0"))
        XCTAssertFalse(ReleaseVersion.isNewer("0.4.0-rc.1", than: "0.3.0"))
        XCTAssertFalse(ReleaseVersion.isNewer("garbage", than: "0.3.0"))
    }

    func testNoticeNamesBothVersions() {
        XCTAssertEqual(UpdateNotice.text(app: "Good Night", new: "0.4.0", current: "0.3.0"),
                       "Good Night 0.4.0 is available — you have 0.3.0.")
        XCTAssertEqual(UpdateNotice.reminder(app: "Good Night", new: "0.4.0"), "Good Night 0.4.0 is available")
    }

    func testRequestsCarryOnlyAFixedUserAgent() {
        XCTAssertEqual(UpdatePolicy.userAgent, "goodnight-update-check")
        XCTAssertFalse(UpdatePolicy.userAgent.contains(where: \.isNumber), "no version in the User-Agent")
    }

    func testCIAndTheEnvOptOutSilenceBackgroundChecks() {
        XCTAssertTrue(UpdatePolicy.backgroundChecksAllowed(environment: [:]))
        XCTAssertFalse(UpdatePolicy.backgroundChecksAllowed(environment: ["CI": "true"]))
        XCTAssertFalse(UpdatePolicy.backgroundChecksAllowed(environment: ["GOODNIGHT_NO_UPDATE_CHECK": "1"]))
        XCTAssertTrue(UpdatePolicy.backgroundChecksAllowed(environment: ["GOODNIGHT_NO_UPDATE_CHECK": "0", "CI": "false"]))
    }

    func testMenuBarOnlyUseGetsAReminderInsteadOfADialog() {
        XCTAssertFalse(UpdatePolicy.standardDriverHandlesScheduledUpdate(showInDock: false))
        XCTAssertTrue(UpdatePolicy.standardDriverHandlesScheduledUpdate(showInDock: true))
    }
}
