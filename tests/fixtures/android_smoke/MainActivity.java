package org.freelane.smoke;

import android.app.Activity;
import android.os.Bundle;
import android.view.Gravity;
import android.widget.TextView;

public final class MainActivity extends Activity {
    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        TextView text = new TextView(this);
        text.setText("FreeLane Android\n\nAPK installed and launched through FreeLane.\n\nAOSP acceptance fixture.");
        text.setTextSize(24);
        text.setTextColor(0xff232c41);
        text.setBackgroundColor(0xfff7f9fc);
        text.setPadding(40, 64, 40, 64);
        text.setGravity(Gravity.CENTER_VERTICAL);
        setContentView(text);
    }
}
