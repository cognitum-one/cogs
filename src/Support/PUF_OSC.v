//==============================================================================================================================
//
//   Copyright © 2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name          : Cognitum
//
//   Description           : Ring Oscillator for RO-PUF
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//   NOTES:
//   Positive edge clock and positive edge asynchronous reset
//   Modelled after SDFFYRPQ2D module from the ARM 12LP+ library
//
//   *** ENSURE THAT THE 'reset' COMES FORM A SYNCHRONOUS SOURCE ***
//
//===============================================================================================================================

`timescale  1ps/10fs

module PUF_OSC (
   output wire Y,
   input  wire RUN,
   input  wire EN
   );

parameter   DELTA = 256;

//real     NOMFREQ  =  8.0;
//realtime center   =  1000/NOMFREQ;
//realtime tYA      =  3.0;
//realtime floor    = (center/2) - (16 * tYA) - (256 * 0.01);
//realtime tYAd     =  floor + (DELTA * 0.01);
//realtime period   =  2 * (16 * tYA +  tYAd);
//realtime netfreq  =  1000 / period;

realtime NOMFREQ  =  8.0;
realtime center   =  125.0;
realtime tYA      =  3.0;
realtime floor    = 62.5 - 48.0 - 2.56; //11.94
realtime tYAd     =  11.94 + (DELTA * 0.01);

wire    a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q;
wire    A,B,C,D,E,F,G,H,I,J,K,L,M,N,O,P,Q;

bufif1   i0  (A,q,EN);  not  #tYAd i1  (a,A);
bufif1   i2  (B,a,EN);  not  #tYA  i3  (b,B);
bufif1   i4  (C,b,EN);  not  #tYA  i5  (c,C);
bufif1   i6  (D,c,EN);  not  #tYA  i7  (d,D);
bufif1   i8  (E,d,EN);  not  #tYA  i9  (e,E);
bufif1   i10 (F,e,EN);  not  #tYA  i11 (f,F);
bufif1   i12 (G,f,EN);  not  #tYA  i13 (g,G);
bufif1   i14 (H,g,EN);  not  #tYA  i15 (h,H);
bufif1   i16 (I,h,EN);  not  #tYA  i17 (i,I);
bufif1   i18 (J,i,EN);  not  #tYA  i19 (j,J);
bufif1   i20 (K,j,EN);  not  #tYA  i21 (k,K);
bufif1   i22 (L,k,EN);  not  #tYA  i23 (l,L);
bufif1   i24 (M,l,EN);  not  #tYA  i25 (m,M);
bufif1   i26 (N,m,EN);  nand #tYA  i27 (n,N,RUN);
bufif1   i28 (O,n,EN);  not  #tYA  i29 (o,O);
bufif1   i30 (P,o,EN);  not  #tYA  i31 (p,P);
bufif1   i32 (Q,p,EN);  not  #tYA  i33 (q,Q);

                        not  #tYA  i34 (Y,n);
endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_PUF_OSC

module t;

defparam mut.DELTA = 512;

wire  OSC;
reg   enable;
reg   run;

PUF_OSC mut (
   .Y    (OSC),
   .RUN  (run),
   .EN   (enable)
   );

initial begin
   run      =  0;
   enable   =  0;
   #1000;
   enable   =  1;
   #1000;
   run      =  1;
   #10000;
   run      =  0;
   #1000;
   enable   =  0;
   #1000;
   enable   =  1;
   #1000;
   run      =  1;
   #10000;
   run      =  0;
   #1000;
   enable   =  0;
   #1000;
   $stop;
   end


endmodule
`endif
/*
 *-------------------------------------------------------------------------------------------------------------------------------
 *
 *
 *    8GHz  125 ps
 *          125 +  delta
 *          125 + (512 * 0.01) = 130.12 ps -> 7.68521365 GHz
 *
 *-------------------------------------------------------------------------------------------------------------------------------
 *
 *  time to count 65536 @ 125       = 8192000
 *                65536 @ 130.12    = 8527544
 *
 *    SO 125 GETS THERE FIRST (natch!)  So what will the count of the 130.12 period be when the other overflows
 *
 *                 8192000 / 130.12 = 62957
 *
 *    Difference is 65536 - 62957   =  2579  or  ~5 per step
 *
 *-------------------------------------------------------------------------------------------------------------------------------
 *
 *  time to count  4294967296 @ 125     =  536,870,912,000
 *                 4294967296 @ 130.12  =  558,861,144,556
 *
 *    SO 125 GETS THERE FIRST (natch!)  So what will the count of the 130.12 period be when the other overflows
 *
 *                 536,870,912,000 / 130.12 = 4,125,967,660.62
 *
 *    Difference is 4,294,967,296 - 4,125,967,660.62   =  168,999,635  or ~330,077 per step
 *
 *-------------------------------------------------------------------------------------------------------------------------------
 *
 *  time to count   2^24 16,777,216 @ 125     =  2097152000
 *                       16,777,216 @ 130.12  =  2183051345
 *
 *    SO 125 GETS THERE FIRST (natch!)  So what will the count of the 130.12 period be when the other overflows
 *
 *                    2,097,152,000 / 130.12  =  16,117,061
 *
 *    Difference is  16,777,216 - 16,117,061  =  660,155   or ~1289 per step
 *
 *-------------------------------------------------------------------------------------------------------------------------------
 *
 *  time to count   2^24 16,777,216 @ 125     =  2097152000
 *                       16,777,216 @ 125.01   =  2097319772
 *
 *    SO 125 GETS THERE FIRST (natch!)  So what will the count of the 130.12 period be when the other overflows
 *
 *                    2,097,152,000 / 125.01 =  16775873
 *
 *    Difference is   16,777,216 - 16,775,873 =  1342
 *
 *-------------------------------------------------------------------------------------------------------------------------------
 */
